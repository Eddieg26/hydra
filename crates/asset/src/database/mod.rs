use crate::{
    asset::{Asset, AssetType, ErasedAsset, ErasedId},
    database::{
        library::AssetLibrary,
        scanner::{AssetScanError, Blacklist, ScanInfo},
    },
    ext::PathExt,
    io::{
        AsyncIoError,
        path::{AssetPath, LoadPath},
    },
};
use ecs::{
    CommandBuffer, Commands, Event, EventWriter, IndexDag, SystemArg,
    core::{
        ImmutableIndexDag,
        task::{IoTaskPool, Task},
    },
};
use futures::TryFutureExt;
use smol::{
    channel::{Receiver, Sender, unbounded},
    future::FutureExt,
    lock::RwLock,
};
use std::{
    collections::{HashMap, HashSet},
    error::Error,
    sync::OnceLock,
};

pub mod commands;
pub mod config;
pub mod library;
pub mod scanner;
pub mod state;

pub use commands::*;
pub use config::*;
pub use state::*;

static DB: OnceLock<AssetDatabase> = OnceLock::new();

pub struct AssetDatabase {
    config: AssetDatabaseConfig,
    library: RwLock<AssetLibrary>,
    states: RwLock<AssetStates>,
    writer: RwLock<Option<()>>,
    sender: Sender<AssetDatabaseEvent>,
    receiver: Receiver<AssetDatabaseEvent>,
}

impl AssetDatabase {
    pub fn init(builder: AssetDatabaseBuilder) -> &'static AssetDatabase {
        DB.get_or_init(|| Self::new(builder))
    }

    pub fn get() -> &'static AssetDatabase {
        DB.get()
            .expect("AssetDatabase not initialized. Make sure to add AssetPlugin to your app.")
    }

    pub fn is_initialized() -> bool {
        DB.get().is_some()
    }

    fn new(builder: AssetDatabaseBuilder) -> AssetDatabase {
        let (sender, receiver) = unbounded();

        Self {
            config: builder.config,
            library: RwLock::default(),
            states: RwLock::default(),
            writer: RwLock::new(Some(())),
            sender,
            receiver,
        }
    }

    pub fn config(&self) -> &AssetDatabaseConfig {
        &self.config
    }

    pub fn registry(&self) -> &AssetRegistry {
        self.config.registry()
    }

    pub fn cache(&self) -> &AssetCache {
        self.config.cache()
    }

    pub fn sources(&self) -> &AssetSources {
        self.config.sources()
    }

    pub async fn send_event(&self, event: impl Into<AssetDatabaseEvent>) {
        let _ = self.sender.send(event.into()).await;
    }

    pub fn update(&self, mut on_event: impl FnMut(AssetDatabaseEvent)) {
        while let Ok(event) = self.receiver.try_recv() {
            on_event(event);
        }
    }

    pub(crate) fn update_database(
        db: &AssetDatabase,
        mut errors: EventWriter<AssetDatabaseError>,
        mut commands: Commands,
    ) {
        db.update(|event| match event {
            AssetDatabaseEvent::ImportError(error) => {
                errors.send(AssetDatabaseError::Import(error))
            }
            AssetDatabaseEvent::LoadError(error) => errors.send(AssetDatabaseError::Load(error)),
            AssetDatabaseEvent::AssetScanError(error) => {
                errors.send(AssetDatabaseError::Scan(error))
            }
            AssetDatabaseEvent::RemovedAssets(ids) => commands.add(RemoveAssets::from(ids)),
            AssetDatabaseEvent::ImportedAsset(id) => db.reload(id),
            AssetDatabaseEvent::LoadedAsset {
                id,
                asset,
                metadata,
                loaded,
            } => {
                commands.add(move |world: &mut ecs::World| {
                    let db = AssetDatabase::get();
                    let meta = db.registry().get(metadata.ty);
                    meta.add(world, id, asset);

                    for (id, ty) in loaded {
                        let meta = db.registry().get(ty);
                        meta.loaded(world, id);
                    }
                });
            }
            AssetDatabaseEvent::AddCommands(buffer) => commands.add_buffer(buffer),
        });
    }

    async fn setup(&self) {
        for source in self.config.sources().iter() {
            let _ = source.create_dir("".as_ref()).await;
        }

        let _ = self.cache().fs().create_dir_all("".as_ref()).await;
        let _ = self
            .cache()
            .fs()
            .create_dir_all(ArtifactPath::Cache.as_str().as_ref())
            .await;
        let _ = self
            .cache()
            .fs()
            .create_dir_all(ArtifactPath::Source.as_str().as_ref())
            .await;

        let library = self.cache().load_library().await.unwrap_or_default();
        self.library.write().await.replace(library);
    }

    pub fn import(&self) -> ImportTask {
        let task = IoTaskPool::get().spawn(async move {
            async fn inner() -> Result<(), AssetScanError> {
                let db = AssetDatabase::get();
                let mut lock = db.writer.write().await;
                db.setup().await;

                let mut blacklist = HashSet::new();
                let mut scan = db.scan(&blacklist).await?;

                while !scan.is_empty() {
                    let ScanInfo { paths, removed } = std::mem::take(&mut scan);

                    db.remove_assets(removed).await;

                    let assets = db.import_assets(paths, &mut blacklist).await;

                    db.process_assets(assets, &mut blacklist).await;

                    scan = db.scan(&blacklist).await?;
                }

                let library = db.library.read().await;
                let _ = db.cache().save_library(&library).await;
                let _ = db.cache().remove_artifacts(ArtifactPath::Source).await;

                Ok(lock.take().unwrap_or(()))
            }

            if let Err(error) = inner().await {
                AssetDatabase::get().send_event(error).await;
            }
        });

        ImportTask(Some(task))
    }

    async fn import_assets(
        &self,
        paths: Vec<AssetPath<'static>>,
        blacklist: &mut Blacklist,
    ) -> ImmutableIndexDag<ErasedId> {
        let mut process = IndexDag::new();
        let mut map = HashMap::new();

        for path in paths {
            let Some(artifacts) = self.import_asset(path).await else {
                continue;
            };

            let artifacts = match artifacts {
                Ok(artifacts) => artifacts,
                Err(error) => {
                    error.path().and_then(|p| Some(blacklist.insert(p.clone())));
                    self.send_event(error).await;
                    continue;
                }
            };

            for artifact in artifacts {
                let node = *map
                    .entry(artifact.id())
                    .or_insert(process.add_node(artifact.id()));

                for dep in &artifact.meta.dependencies {
                    let dependency = *map.entry(*dep).or_insert(process.add_node(*dep));
                    process.add_dependency(dependency, node);
                }

                for child in &artifact.meta.children {
                    let dependency = *map.entry(*child).or_insert(process.add_node(*child));
                    process.add_dependency(dependency, node);
                }

                let mut library = self.library.write().await;
                library.add_asset(artifact.id(), artifact.path().clone());
            }
        }

        process.build_immutable().unwrap()
    }

    async fn import_asset(
        &self,
        path: AssetPath<'static>,
    ) -> Option<Result<Vec<Artifact>, ImportError>> {
        let ext = path.ext()?;
        let fs = self.config.sources().get(path.source())?;
        let importer = self.config.importers().get_by_ext(ext)?;
        let ty = self.registry().get_ty(importer.asset_type())?;

        let mut reader = match fs.reader(&path).await {
            Ok(reader) => reader,
            Err(error) => {
                let error = Box::new(error);
                return Some(Err(ImportError::ImportAsset { path, error }));
            }
        };

        let settings = fs
            .read(&path.append_ext("meta"))
            .await
            .and_then(|b| importer.deserialize_settings(&b))
            .unwrap_or(importer.default_settings());

        let ctx = ImportContext::new(fs, self.registry(), &path, settings.id(), ty);
        let artifacts = match importer.import(ctx, &mut reader, settings.as_ref()).await {
            Ok(artifacts) => artifacts,
            Err(error) => return Some(Err(ImportError::ImportAsset { path, error })),
        };

        let mut saved = Vec::new();
        for artifact in artifacts {
            if let Err(error) = self
                .cache()
                .save_artifact(ArtifactPath::Source, &artifact)
                .await
            {
                return Some(Err(ImportError::SaveAsset {
                    path: artifact.meta.path,
                    error,
                    cache: ArtifactPath::Source,
                }));
            }

            saved.push(artifact);
        }

        Some(Ok(saved))
    }

    async fn process_assets(&self, assets: ImmutableIndexDag<ErasedId>, blacklist: &mut Blacklist) {
        for id in assets.iter() {
            match self.process_asset(id).await {
                Ok(id) => self.send_event(AssetDatabaseEvent::ImportedAsset(id)).await,
                Err(error) => {
                    error.path().and_then(|p| Some(blacklist.insert(p.clone())));
                    self.send_event(error).await;
                }
            };
        }
    }

    async fn process_asset(&self, id: &ErasedId) -> Result<ErasedId, ImportError> {
        let mut artifact = match self.cache().read_artifact(ArtifactPath::Source, id).await {
            Ok(artifact) => artifact,
            Err(error) => {
                let library = self.library.read().await;
                let path = library.path(id).cloned();
                return Err(ImportError::ProcessAsset {
                    id: *id,
                    path,
                    error: Box::new(error),
                });
            }
        };

        let processor = match artifact.meta.import.processor {
            Some(processor) => Some(self.config.processors().get(processor)),
            None => self.config.processors().get_default(artifact.ty()),
        };

        let process = async {
            let processor = processor?;
            let output_ty = self.registry().get_ty(processor.output_asset())?;
            let library = self.library.read().await;
            let mut ctx =
                ProcessContext::new(&library, self.cache(), artifact.path(), artifact.ty());
            let data = match processor.process(&mut ctx, artifact.data()).await {
                Ok(data) => data,
                Err(error) => {
                    return Some(Err(ImportError::ProcessAsset {
                        id: *id,
                        path: Some(artifact.path().clone()),
                        error,
                    }));
                }
            };

            let deps = ctx.dependencies;
            artifact.meta.ty = output_ty;
            artifact.data = data;
            Some(Ok(deps))
        };

        let deps = match process.await {
            Some(deps) => deps?,
            None => vec![],
        };

        artifact.meta.import.set_dependencies(deps);

        if let Err(error) = self
            .cache()
            .save_artifact(ArtifactPath::Cache, &artifact)
            .await
        {
            return Err(ImportError::SaveAsset {
                path: artifact.meta.path,
                error,
                cache: ArtifactPath::Cache,
            });
        }

        Ok(artifact.id())
    }

    async fn remove_assets(&self, mut paths: Vec<AssetPath<'static>>) {
        let mut removed = Vec::new();
        while let Some(path) = paths.pop() {
            let mut library = self.library.write().await;
            let Some(id) = library.remove_asset(&path) else {
                continue;
            };

            removed.push(id);

            let meta = self
                .cache()
                .artifact_reader(ArtifactPath::Cache, &id)
                .and_then(|r| r.read_metadata())
                .await;

            let _ = self.cache().remove_artifact(ArtifactPath::Cache, &id).await;

            if let Ok(meta) = meta {
                let children = meta.children.iter().filter_map(|c| library.path(&c));
                paths.extend(children.cloned());
            }
        }

        if !removed.is_empty() {
            self.send_event(AssetDatabaseEvent::RemovedAssets(removed))
                .await;
        }
    }

    pub fn load<A: Asset>(&self, path: impl Into<LoadPath<'static>>) -> LoadTask {
        let (sender, receiver) = smol::channel::bounded(1);
        let task = LoadTask::new(receiver);
        let path: LoadPath<'static> = path.into();

        smol::spawn(async move {
            async fn inner(path: LoadPath<'static>) -> Result<ErasedId, LoadError> {
                let db = AssetDatabase::get();
                let _ = db.writer.read().await;

                let id = match path {
                    LoadPath::Path(path) => {
                        let library = db.library.read().await;
                        library
                            .get(&path)
                            .ok_or(LoadError::from(LoadPath::from(path)))
                            .copied()?
                    }
                    LoadPath::Id(id) => id,
                };

                let mut stack = vec![id];
                while let Some(id) = stack.pop() {
                    let (asset, metadata, loaded) = match db.load_asset(id).await {
                        Ok(result) => result,
                        Err(error) => {
                            let mut states = db.states.write().await;
                            states.failed(id);
                            db.send_event(error).await;
                            continue;
                        }
                    };

                    let states = db.states.read().await;
                    for dependency in metadata.all_dependencies().copied() {
                        let state = states.get_load_state(dependency);
                        if matches!(state, LoadState::Unloaded | LoadState::Failed) {
                            stack.push(dependency);
                        }
                    }

                    let event = AssetDatabaseEvent::LoadedAsset {
                        id,
                        asset,
                        metadata,
                        loaded,
                    };

                    db.send_event(event).await;
                }

                Ok(id)
            }

            let _ = sender.send(inner(path).await).await;
        })
        .detach();

        task
    }

    async fn load_asset(
        &self,
        id: ErasedId,
    ) -> Result<(ErasedAsset, AssetMetadata, Vec<(ErasedId, AssetType)>), LoadError> {
        let mut states = self.states.write().await;
        states.loading(id);

        let artifact = self
            .cache()
            .read_artifact(ArtifactPath::Cache, &id)
            .await
            .map_err(|error| LoadError::Io { id, error })?;

        let meta = self.registry().get(artifact.ty());
        let asset = meta
            .deserialize(artifact.data())
            .ok_or(LoadError::MissingDeserializer { name: meta.name })?
            .map_err(|error| LoadError::Io {
                id,
                error: AsyncIoError::from(error),
            })?;

        let loaded = states.loaded(
            id,
            artifact.ty(),
            &artifact.meta().dependencies,
            artifact.meta.parent,
        );

        Ok((asset, artifact.meta, loaded))
    }

    pub fn reload(&self, id: ErasedId) {
        IoTaskPool::get().spawn(async move {
            let db = AssetDatabase::get();
            if db.states.read().await.get_load_state(id).can_reload() {
                let (asset, metadata, loaded) = match db.load_asset(id).await {
                    Ok(v) => v,
                    Err(error) => return db.send_event(error).await,
                };

                let event = AssetDatabaseEvent::LoadedAsset {
                    id,
                    asset,
                    metadata,
                    loaded,
                };

                db.send_event(event).await;
            }
        });
    }
}

unsafe impl SystemArg for &AssetDatabase {
    type Item<'world, 'state> = &'static AssetDatabase;

    type State = &'static AssetDatabase;

    fn init(world: &mut ecs::World, _: &mut ecs::WorldAccess) -> Self::State {
        if AssetDatabase::is_initialized() {
            AssetDatabase::get()
        } else if let Some(builder) = world.remove_resource::<AssetDatabaseBuilder>() {
            AssetDatabase::init(builder)
        } else {
            panic!("AssetDatabase not initialized and no AssetConfigBuilder found in world");
        }
    }

    unsafe fn get<'world, 'state>(
        state: &'state mut Self::State,
        _: ecs::world::WorldCell<'world>,
        _: &ecs::SystemMeta,
    ) -> Self::Item<'world, 'state> {
        state
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ImportError {
    #[error("Asset scan error: {error}")]
    ScanError { error: AssetScanError },

    #[error("Failed to import asset: {path}, error: {error}")]
    ImportAsset {
        path: AssetPath<'static>,
        error: Box<dyn Error + Send + Sync>,
    },

    #[error("Failed to process asset: {:?} {:?}, error: {}", id, path, error)]
    ProcessAsset {
        id: ErasedId,
        path: Option<AssetPath<'static>>,
        error: Box<dyn Error + Send + Sync>,
    },

    #[error("Failed to save asset: {path} at cache: {cache}, error: {error}")]
    SaveAsset {
        path: AssetPath<'static>,
        error: AsyncIoError,
        cache: ArtifactPath,
    },
}

impl ImportError {
    pub fn path(&self) -> Option<&AssetPath<'static>> {
        match self {
            ImportError::ScanError { error } => error.path(),
            ImportError::ImportAsset { path, .. } => Some(path),
            ImportError::ProcessAsset { path, .. } => path.as_ref(),
            ImportError::SaveAsset { path, .. } => Some(path),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum LoadError {
    #[error("Asset not found at path: {path}")]
    AssetNotFound { path: LoadPath<'static> },

    #[error("Failed to load asset: {:?}, error: {error}", id)]
    Io { id: ErasedId, error: AsyncIoError },

    #[error("Deserializer for asset {name} not registered")]
    MissingDeserializer { name: &'static str },
}

impl From<LoadPath<'static>> for LoadError {
    fn from(path: LoadPath<'static>) -> Self {
        Self::AssetNotFound { path }
    }
}

#[derive(Debug, Event, thiserror::Error)]
pub enum AssetDatabaseError {
    #[error("{0}")]
    Scan(AssetScanError),
    #[error("{0}")]
    Import(ImportError),
    #[error("{0}")]
    Load(LoadError),
}

pub enum AssetDatabaseEvent {
    AssetScanError(AssetScanError),
    LoadError(LoadError),
    ImportError(ImportError),
    RemovedAssets(Vec<ErasedId>),
    ImportedAsset(ErasedId),
    LoadedAsset {
        id: ErasedId,
        asset: ErasedAsset,
        metadata: AssetMetadata,
        loaded: Vec<(ErasedId, AssetType)>,
    },
    AddCommands(CommandBuffer),
}

impl From<AssetScanError> for AssetDatabaseEvent {
    fn from(value: AssetScanError) -> Self {
        Self::AssetScanError(value)
    }
}

impl From<ImportError> for AssetDatabaseEvent {
    fn from(value: ImportError) -> Self {
        Self::ImportError(value)
    }
}

impl From<LoadError> for AssetDatabaseEvent {
    fn from(value: LoadError) -> Self {
        Self::LoadError(value)
    }
}

impl From<CommandBuffer> for AssetDatabaseEvent {
    fn from(value: CommandBuffer) -> Self {
        Self::AddCommands(value)
    }
}

pub struct ImportTask(Option<Task<()>>);

impl ImportTask {
    pub fn detach(mut self) {
        if let Some(task) = self.0.take() {
            task.detach();
        }
    }

    pub async fn cancel(mut self) -> Option<()> {
        if let Some(task) = self.0.take() {
            task.cancel().await
        } else {
            None
        }
    }

    pub fn is_finished(&self) -> bool {
        if let Some(task) = &self.0 {
            task.is_finished()
        } else {
            true
        }
    }
}

impl Future for ImportTask {
    type Output = ();

    fn poll(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        if let Some(task) = self.0.as_mut() {
            task.poll(cx)
        } else {
            std::task::Poll::Ready(())
        }
    }
}

impl Drop for ImportTask {
    fn drop(&mut self) {
        if let Some(task) = self.0.take() {
            task.detach();
        }
    }
}

#[derive(Clone)]
pub struct LoadTask {
    receiver: Receiver<Result<ErasedId, LoadError>>,
}

impl LoadTask {
    fn new(receiver: Receiver<Result<ErasedId, LoadError>>) -> Self {
        Self { receiver }
    }
}

impl Future for LoadTask {
    type Output = Result<ErasedId, LoadError>;

    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        match self.receiver.try_recv() {
            Ok(result) => std::task::Poll::Ready(result),
            Err(_) => {
                cx.waker().wake_by_ref();
                std::task::Poll::Pending
            }
        }
    }
}

#[allow(unused_imports, dead_code)]
mod tests {
    use crate::{
        asset::{Asset, AssetDependencies, ErasedAsset},
        database::{
            AssetDatabase, AssetDatabaseEvent,
            config::{
                AssetDatabaseBuilder,
                importer::{AssetImporter, AssetProcessor},
            },
        },
        io::{
            FileSystem,
            embedded::EmbeddedFs,
            path::{AssetPath, AssetSource},
            vfs::VirtualFs,
        },
        settings::DefaultSettings,
    };
    use ecs::core::task::{IoTaskPool, TaskPoolBuilder};
    use serde::{Deserialize, Serialize};
    use smol::io::{AsyncReadExt, AsyncWriteExt};

    #[derive(Debug, Serialize, Deserialize)]
    pub struct TextFile(String);
    impl Asset for TextFile {}
    impl AssetDependencies for TextFile {}

    impl AssetImporter for TextFile {
        type Asset = Self;

        type Settings = DefaultSettings;

        type Error = std::io::Error;

        async fn import(
            _: &mut super::config::importer::ImportContext<'_>,
            reader: &mut dyn crate::io::AsyncReader,
            _: &crate::settings::AssetSettings<Self::Settings>,
        ) -> Result<Self::Asset, Self::Error> {
            let mut content = String::new();
            reader.read_to_string(&mut content).await?;

            Ok(Self(content))
        }

        fn extensions() -> &'static [&'static str] {
            &["txt"]
        }
    }

    impl AssetProcessor for TextFile {
        type Input = Self;

        type Output = Self;

        type Error = std::io::Error;

        async fn process(
            _: &mut super::config::importer::ProcessContext<'_>,
            mut asset: Self::Input,
        ) -> Result<Self::Output, Self::Error> {
            asset.0 = String::from("processed");
            Ok(asset)
        }
    }

    fn setup() -> &'static AssetDatabase {
        IoTaskPool::init(TaskPoolBuilder::new().build());
        let source = smol::block_on(async {
            let source = VirtualFs::new();
            let mut writer = source.writer("test.txt".as_ref()).await.unwrap();
            writer.write_all(b"test").await.unwrap();

            source
        });

        let mut builder = AssetDatabaseBuilder::new();
        builder.add_source(AssetSource::Default, source);
        builder.set_cache(VirtualFs::new());
        builder.add_importer::<TextFile>();
        builder.set_default_processor::<TextFile>();

        AssetDatabase::init(builder)
    }

    #[test]
    fn test_import() {
        let database = setup();
        smol::block_on(database.import());

        let library = database.library.try_read().unwrap();
        let id = library.get(&AssetPath::from("test.txt")).copied().unwrap();

        database.update(|event| match event {
            AssetDatabaseEvent::AssetScanError(error) => panic!("{error}"),
            AssetDatabaseEvent::ImportError(error) => panic!("{error}"),
            AssetDatabaseEvent::LoadError(error) => panic!("{error}"),
            AssetDatabaseEvent::RemovedAssets(ids) => panic!("Assets Removed: {:?}", ids),
            AssetDatabaseEvent::LoadedAsset { id, .. } => panic!("Asset Loaded: {:?}", id),
            AssetDatabaseEvent::ImportedAsset(imported) => assert_eq!(imported, id),
            _ => {}
        });
    }

    #[test]
    fn test_load() {
        let database = setup();
        smol::block_on(database.import());

        let loaded = smol::block_on(database.load::<TextFile>("test.txt")).unwrap();
        database.update(|event| match event {
            AssetDatabaseEvent::AssetScanError(error) => panic!("{error}"),
            AssetDatabaseEvent::ImportError(error) => panic!("{error}"),
            AssetDatabaseEvent::LoadError(error) => panic!("{error}"),
            AssetDatabaseEvent::RemovedAssets(ids) => panic!("Assets Removed: {:?}", ids),
            AssetDatabaseEvent::LoadedAsset { id, asset, .. } => {
                let file = unsafe { asset.into::<TextFile>() };
                assert_eq!(loaded, id);
                assert_eq!(file.0, "processed");
            }
            _ => {}
        });
    }
}
