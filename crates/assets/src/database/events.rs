use crate::{
    asset::{AssetMetadata, ErasedId, Folder},
    config::{AssetConfig, BoxedError, importer::ImportContext, processor::ProcessContext},
    io::{
        ArtifactMeta, AssetIoError, AssetLibrary, AssetPath, AssetSource, SourceName, deserialize,
    },
};
use ecs::{Command, Event, Events, IndexDag, core::ImmutableIndexDag};
use smol::{
    block_on,
    channel::Sender,
    lock::{Mutex, RwLock},
    stream::StreamExt,
};
use std::{
    collections::HashMap,
    hash::Hash,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use super::{
    AssetDatabase,
    state::{AssetStates, LoadState},
};

#[derive(Default)]
pub struct ImportInfo {
    imported: Vec<AssetPath<'static>>,
    removed: Vec<AssetPath<'static>>,
}

#[derive(thiserror::Error, Debug)]
pub enum ImportError {
    #[error("{0}")]
    File(AssetIoError),

    #[error("{0}")]
    Folder(AssetIoError),

    #[error("Source: {name} {error}")]
    Source {
        name: SourceName<'static>,
        error: AssetIoError,
    },

    #[error("{0}")]
    LoadAsset(AssetIoError),

    #[error("{0}")]
    LoadArtifact(AssetIoError),

    #[error("{0}")]
    LoadMetadata(AssetIoError),

    #[error("{0}")]
    ImportAsset(BoxedError),

    #[error("{0}")]
    SaveAsset(AssetIoError),

    #[error("{0}")]
    ProcessAsset(BoxedError),

    #[error("{0}")]
    Unknown(AssetIoError),
}

impl Event for ImportError {}

impl Command for ImportError {
    fn execute(self, world: &mut ecs::World) {
        let events = world.resource_mut::<Events<ImportError>>();
        events.writer().send(self);
    }
}

pub enum DatabaseEvent {
    ReloadAsset(ErasedId),
    UnloadAsset(ErasedId),
    ImportError(ImportError),
}

impl From<ImportError> for DatabaseEvent {
    fn from(value: ImportError) -> Self {
        DatabaseEvent::ImportError(value)
    }
}

pub trait DatabaseCommand: Send + Sync + 'static {
    fn execute(&self, database: AssetDatabase) -> impl Future<Output = ()>;
}

pub type BoxedCommand = Box<dyn Fn(AssetDatabase) + Send>;

#[derive(Clone, Default)]
pub struct DatabaseCommands {
    commands: Arc<Mutex<Vec<BoxedCommand>>>,
    running: Arc<AtomicBool>,
}

impl DatabaseCommands {
    pub fn execute<C: DatabaseCommand>(&self, command: C, database: AssetDatabase) {
        self.push(command);
        self.run(database);
    }

    fn run(&self, database: AssetDatabase) {
        if self.running.swap(true, Ordering::SeqCst) {
            return;
        }

        let commands = self.clone();

        smol::spawn(async move {
            while let Some(command) = commands.pop() {
                command(database.clone());
            }

            commands.running.swap(false, Ordering::SeqCst);
        })
        .detach();
    }

    fn push<C: DatabaseCommand>(&self, command: C) {
        let mut commands = self.commands.lock_blocking();
        let command = move |database| block_on(command.execute(database));

        commands.push(Box::new(command));
    }

    fn pop(&self) -> Option<BoxedCommand> {
        let mut commands = self.commands.lock_blocking();

        commands.pop()
    }
}

pub struct ImportAssets;

impl DatabaseCommand for ImportAssets {
    async fn execute(&self, database: AssetDatabase) {
        let AssetDatabase {
            config,
            library,
            sender,
            states,
            ..
        } = database;

        if let Err(error) = config.cache().create_temp().await {
            let _ = sender.send(DatabaseEvent::ImportError(ImportError::Unknown(error)));
            return;
        }

        loop {
            let mut library = library.write().await;
            let mut imported = vec![];
            let mut removed = vec![];

            for (name, source) in config.sources().iter() {
                match self
                    .import_source(name, source, &config, &library, &sender)
                    .await
                {
                    Ok(info) => {
                        imported.extend(info.imported);
                        removed.extend(info.removed);
                    }
                    Err(error) => {
                        let error = ImportError::Source {
                            name: name.into_owned(),
                            error,
                        };

                        let _ = sender.send(error.into()).await;
                    }
                }
            }

            if imported.is_empty() && removed.is_empty() {
                break;
            }

            self.remove_assets(removed, &config, &mut library, &sender)
                .await;

            let process_list = self
                .import_assets(imported, &config, &mut library, &sender)
                .await;

            self.process_assets(process_list, &config, &sender, states.clone())
                .await;
        }

        let _ = config.cache().delete_temp().await;
    }
}

impl ImportAssets {
    pub async fn import_source<'a>(
        &'a self,
        name: &'a SourceName<'a>,
        source: &'a AssetSource,
        config: &'a AssetConfig,
        library: &'a AssetLibrary,
        events: &'a Sender<DatabaseEvent>,
    ) -> Result<ImportInfo, AssetIoError> {
        let path = AssetPath::new(name.clone(), "");
        let ImportInfo {
            mut imported,
            mut removed,
        } = self.import_dir(path, source).await?;

        while let Some(path) = imported.pop() {
            match source.is_dir(path.path()).await {
                Ok(true) => match self.import_dir(path, source).await {
                    Ok(info) => {
                        imported.extend(info.imported);
                        removed.extend(info.removed);
                    }
                    Err(error) => {
                        let _ = events.send(ImportError::Folder(error).into()).await;
                    }
                },
                Ok(false) => {
                    if matches!(path.ext(), Some("meta") | None) {
                        continue;
                    }

                    match self.import_file(path, source, config, library).await {
                        Ok(Some(path)) => imported.push(path.into_static()),
                        Ok(None) => continue,
                        Err(error) => {
                            let _ = events.send(ImportError::File(error).into()).await;
                        }
                    }
                }
                Err(error) => {
                    let _ = events.send(ImportError::Unknown(error).into()).await;
                }
            }
        }

        Ok(ImportInfo { imported, removed })
    }

    pub async fn import_dir<'a>(
        &'a self,
        path: AssetPath<'a>,
        source: &'a AssetSource,
    ) -> Result<ImportInfo, AssetIoError> {
        let mut metadata = match source.read_metadata_bytes(path.path()).await {
            Ok(data) => deserialize(&data).unwrap_or_default(),
            Err(_) => AssetMetadata::<Folder>::default(),
        };

        let mut paths = source
            .read_dir(path.path())
            .await?
            .collect::<Vec<_>>()
            .await;

        let removed = metadata
            .children
            .iter()
            .filter_map(|c| {
                (!paths.contains(c))
                    .then_some(AssetPath::new(path.source().into_owned(), c.clone()))
            })
            .collect::<Vec<_>>();

        metadata.children = paths.clone();

        let imported = paths
            .drain(..)
            .map(|p| AssetPath::new(path.source().into_owned(), p))
            .collect::<Vec<_>>();

        let _ = source.save_metadata(path.path(), &metadata).await;

        Ok(ImportInfo { imported, removed })
    }

    pub async fn import_file<'a>(
        &'a self,
        path: AssetPath<'a>,
        source: &'a AssetSource,
        config: &'a AssetConfig,
        library: &'a AssetLibrary,
    ) -> Result<Option<AssetPath<'a>>, AssetIoError> {
        let Some(id) = library.get_id(&path).copied() else {
            return Ok(Some(path));
        };

        let meta_path = path.path().with_extension("meta");
        if !source.exists(&meta_path).await? {
            return Ok(Some(path));
        }

        let Ok(mut reader) = config.cache().get_artifact_reader(id).await else {
            return Ok(Some(path));
        };

        let checksum = match Self::get_checksum(source, &path).await? {
            Some(checksum) => checksum,
            None => return Ok(Some(path)),
        };

        let Ok(header) = reader.header().await else {
            return Ok(Some(path));
        };

        let Ok(meta) = reader.meta(&header).await else {
            return Ok(Some(path));
        };

        if checksum != meta.import.checksum {
            return Ok(Some(path));
        }

        if Self::get_full_checksum(config, &meta).await != meta.import.full_checksum {
            return Ok(Some(path));
        }

        Ok(None)
    }

    async fn import_assets<'a>(
        &'a self,
        paths: Vec<AssetPath<'static>>,
        config: &'a AssetConfig,
        library: &'a mut AssetLibrary,
        events: &'a Sender<DatabaseEvent>,
    ) -> ImmutableIndexDag<ErasedId> {
        let mut graph = IndexDag::new();
        let mut node_map = HashMap::new();

        for path in paths {
            let source = config.sources().get(path.source()).unwrap();
            let mut reader = match source.reader(path.path()).await {
                Ok(reader) => reader,
                Err(error) => {
                    let _ = events.send(ImportError::LoadAsset(error).into()).await;
                    continue;
                }
            };

            let Some(importer) = path
                .ext()
                .and_then(|ext| config.importers().get_by_ext(ext))
            else {
                continue;
            };

            let Some(ty) = config.registry().get_ty(importer.asset_type()) else {
                continue;
            };

            let metadata = source
                .read_metadata_bytes(path.path())
                .await
                .and_then(|data| importer.deserialize_metadata(&data))
                .unwrap_or(importer.default_metadata());

            let mut ctx = ImportContext::new(ty, &path, source);

            let artifact = match importer.import(&mut ctx, &mut reader, &metadata).await {
                Ok(artifact) => artifact,
                Err(error) => {
                    let _ = events.send(ImportError::ImportAsset(error).into()).await;
                    continue;
                }
            };

            if let Err(error) = config.cache().save_temp_artifact(&artifact).await {
                let _ = events.send(ImportError::SaveAsset(error).into()).await;
                continue;
            }

            let node = *node_map
                .entry(artifact.id())
                .or_insert_with(|| graph.add_node(artifact.id()));

            for dependency in &artifact.meta().dependencies {
                let dependency = *node_map
                    .entry(*dependency)
                    .or_insert_with(|| graph.add_node(*dependency));

                graph.add_dependency(dependency, node);
            }

            library.add_asset(path, artifact.id());
        }

        let _ = graph.build();

        graph.into_immutable()
    }

    async fn process_assets<'a>(
        &'a self,
        graph: ImmutableIndexDag<ErasedId>,
        config: &'a AssetConfig,
        events: &'a Sender<DatabaseEvent>,
        states: Arc<RwLock<AssetStates>>,
    ) {
        for id in graph.iter().copied() {
            let mut artifact = match config.cache().get_temp_artifact(id).await {
                Ok(artifact) => artifact,
                Err(error) => {
                    let _ = events.send(ImportError::LoadArtifact(error).into()).await;
                    continue;
                }
            };

            if let Some(processor) = artifact.meta().import.processor {
                let source = config.sources().get(artifact.path().source()).unwrap();
                let metadata = match source.read_metadata_bytes(artifact.path().path()).await {
                    Ok(metadata) => metadata,
                    Err(error) => {
                        let _ = events.send(ImportError::LoadMetadata(error).into()).await;
                        continue;
                    }
                };

                let mut ctx = ProcessContext::new(config.cache());
                let processor = config.processors()[processor];
                match processor.process(&mut ctx, artifact.data(), metadata).await {
                    Ok(data) => {
                        artifact.meta.ty =
                            config.registry().get_ty(processor.output_asset()).unwrap();
                        artifact.header.asset = data.len() as u32;
                        artifact.data = data;
                    }
                    Err(error) => {
                        let _ = events.send(ImportError::ProcessAsset(error).into()).await;
                        continue;
                    }
                }
            }

            if let Err(error) = config.cache().save_artifact(&artifact).await {
                let _ = events.send(ImportError::SaveAsset(error).into()).await;
            } else if matches!(
                states.read().await.get_load_state(id),
                LoadState::Loaded | LoadState::Failed
            ) {
                let _ = events.send(DatabaseEvent::ReloadAsset(id));
            }
        }
    }

    async fn remove_assets<'a>(
        &'a self,
        paths: Vec<AssetPath<'static>>,
        config: &'a AssetConfig,
        library: &'a mut AssetLibrary,
        commands: &'a Sender<DatabaseEvent>,
    ) {
        for path in paths {
            let Some(id) = library.remove_asset(&path) else {
                continue;
            };

            let _ = commands.send(DatabaseEvent::UnloadAsset(id));

            let Ok(reader) = config.cache().get_artifact_reader(id).await else {
                let _ = config.cache().remove_artifact(id).await;
                continue;
            };

            let Ok(_) = reader.into_meta().await else {
                let _ = config.cache().remove_artifact(id).await;
                continue;
            };

            let _ = config.cache().remove_artifact(id).await;
        }
    }

    async fn get_checksum<'a>(
        source: &'a AssetSource,
        path: &'a AssetPath<'a>,
    ) -> Result<Option<u32>, AssetIoError> {
        let asset = source.read_asset_bytes(path.path()).await?;
        let metadata = match source.read_metadata_bytes(path.path()).await {
            Ok(metadata) => metadata,
            Err(_) => return Ok(None),
        };

        let mut hasher = crc32fast::Hasher::new();
        asset.hash(&mut hasher);
        metadata.hash(&mut hasher);

        Ok(Some(hasher.finalize()))
    }

    async fn get_full_checksum(config: &AssetConfig, meta: &ArtifactMeta) -> u32 {
        let mut hasher = crc32fast::Hasher::new();
        meta.import.checksum.hash(&mut hasher);

        for dependency in &meta.import.dependencies {
            let Ok(mut reader) = config.cache().get_artifact_reader(*dependency).await else {
                continue;
            };

            let Ok(header) = reader.header().await else {
                continue;
            };

            let Ok(dep_meta) = reader.meta(&header).await else {
                continue;
            };

            dep_meta.import.full_checksum.hash(&mut hasher);
        }

        hasher.finalize()
    }
}

impl Command for ImportAssets {
    fn execute(self, world: &mut ecs::World) {
        world.resource::<AssetDatabase>().execute(self);
    }
}
