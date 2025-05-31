use super::{AssetDatabase, DatabaseEvent, commands::AssetCommand};
use crate::{
    asset::{AssetMetadata, ErasedId, Folder},
    config::{
        importer::{ImportContext, ImportError},
        processor::ProcessContext,
    },
    io::{
        AssetIoError, AssetLibrary, AssetPath, AssetSource, ImportMeta, PathExt, SourceName,
        deserialize,
    },
};
use ecs::{
    IndexDag,
    core::{ImmutableIndexDag, task::IoTaskPool},
};
use smol::stream::StreamExt;
use std::collections::{HashMap, HashSet};

#[derive(Default)]
pub struct ImportInfo {
    imported: Vec<AssetPath<'static>>,
    removed: Vec<AssetPath<'static>>,
}

impl ImportInfo {
    pub fn new() -> Self {
        Self {
            imported: vec![],
            removed: vec![],
        }
    }

    pub fn extend(&mut self, other: Self) {
        self.imported.extend(other.imported);
        self.removed.extend(other.removed);
    }

    pub fn is_empty(&self) -> bool {
        self.imported.is_empty() && self.removed.is_empty()
    }
}

impl AssetDatabase {
    pub fn import(&self) {
        let import = async {
            let db = AssetDatabase::get();

            let _writer = db.writer.write().await;

            if let Err(error) = db.config.cache().create_temp().await {
                db.send_event(DatabaseEvent::ImportError(ImportError::Unknown(error)))
                    .await;
                return;
            }

            let mut skip = HashSet::new();

            loop {
                let mut library = db.library.write().await;
                let mut import_info = ImportInfo::new();

                for (name, source) in db.config.sources().iter() {
                    match db.import_source(name, source, &library, &mut skip).await {
                        Ok(source_info) => import_info.extend(source_info),
                        Err(error) => {
                            let error = ImportError::from((name.into_owned(), error));
                            db.send_event(error).await;
                        }
                    }
                }

                if import_info.is_empty() {
                    break;
                }

                let ImportInfo { imported, removed } = import_info;

                db.remove_assets(removed, &mut library).await;

                let process_list = db.import_assets(imported, &mut library).await;

                db.process_assets(process_list).await;
            }

            let _ = db.config.cache().delete_temp().await;
        };

        IoTaskPool::get().spawn(import).detach();
    }

    async fn import_source<'a>(
        &'a self,
        name: &'a SourceName<'a>,
        source: &'a AssetSource,
        library: &'a AssetLibrary,
        skip: &'a mut HashSet<AssetPath<'static>>,
    ) -> Result<ImportInfo, AssetIoError> {
        let path = AssetPath::new(name.clone(), "");
        let ImportInfo {
            mut imported,
            mut removed,
        } = self.import_dir(path, source).await?;

        let mut import = vec![];

        while let Some(path) = imported.pop() {
            if skip.contains(&path) {
                continue;
            }

            match source.is_dir(path.path()).await {
                Ok(true) => match self.import_dir(path, source).await {
                    Ok(info) => {
                        imported.extend(info.imported);
                        removed.extend(info.removed);
                    }
                    Err(error) => {
                        self.send_event(ImportError::Folder(error)).await;
                    }
                },
                Ok(false) => {
                    let Some(ext) = path.ext() else {
                        continue;
                    };

                    if ext == "meta" || self.config.importers().get_by_ext(ext).is_none() {
                        continue;
                    }

                    match self.import_file(path.clone(), source, library).await {
                        Ok(Some(path)) => import.push(path.into_owned()),
                        Ok(None) => continue,
                        Err(error) => {
                            self.send_event(ImportError::File(error)).await;
                            skip.insert(path.into_owned());
                        }
                    }
                }
                Err(error) => {
                    self.send_event(ImportError::Unknown(error)).await;
                }
            }
        }

        Ok(ImportInfo {
            imported: import,
            removed,
        })
    }

    async fn import_dir<'a>(
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

    async fn import_file<'a>(
        &'a self,
        path: AssetPath<'a>,
        source: &'a AssetSource,
        library: &'a AssetLibrary,
    ) -> Result<Option<AssetPath<'a>>, AssetIoError> {
        let Some(id) = library.get_id(&path).copied() else {
            return Ok(Some(path));
        };

        let meta_path = path.path().append_ext("meta");
        if !source.exists(&meta_path).await? {
            return Ok(Some(path));
        }

        let Ok(mut reader) = self.config.cache().get_artifact_reader(id).await else {
            return Ok(Some(path));
        };

        let Ok(meta) = reader.meta().await else {
            return Ok(Some(path));
        };

        let checksum = match Self::get_checksum(source, &path).await? {
            Some(checksum) => checksum,
            None => return Ok(Some(path)),
        };

        if checksum != meta.import.checksum {
            return Ok(Some(path));
        }

        let dependencies = meta
            .import
            .dependencies
            .iter()
            .map(|(_, checksum)| *checksum);

        let full_checksum = ImportMeta::get_full_checksum(checksum, dependencies);
        if full_checksum != meta.import.full_checksum {
            return Ok(Some(path));
        }

        Ok(None)
    }

    async fn import_assets<'a>(
        &'a self,
        paths: Vec<AssetPath<'static>>,
        library: &'a mut AssetLibrary,
    ) -> ImmutableIndexDag<ErasedId> {
        let mut graph = IndexDag::new();
        let mut node_map = HashMap::new();

        for path in paths {
            let source = self.config.sources().get(path.source()).unwrap();
            let mut reader = match source.reader(path.path()).await {
                Ok(reader) => reader,
                Err(error) => {
                    self.send_event(ImportError::LoadAsset(error)).await;
                    continue;
                }
            };

            let Some(importer) = path
                .ext()
                .and_then(|ext| self.config.importers().get_by_ext(ext))
            else {
                continue;
            };

            let Some(ty) = self.config.registry().get_ty(importer.asset_type()) else {
                continue;
            };

            let metadata = source
                .read_metadata_bytes(path.path())
                .await
                .and_then(|data| importer.deserialize_metadata(&data))
                .unwrap_or(importer.default_metadata());

            let ctx = ImportContext::new(
                metadata.erased_id(),
                ty,
                &path,
                source,
                self.config.registry(),
            );

            let artifacts = match importer.import(ctx, &mut reader, &metadata).await {
                Ok(artifacts) => artifacts,
                Err(error) => {
                    self.send_event(ImportError::ImportAsset(error)).await;
                    continue;
                }
            };

            for artifact in artifacts {
                if let Err(error) = self.config.cache().save_temp_artifact(&artifact).await {
                    self.send_event(ImportError::SaveAsset(error)).await;
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

                let id = artifact.id();

                library.add_asset(artifact.meta.path, id);
            }
        }

        let _ = graph.build();

        graph.into_immutable()
    }

    async fn process_assets<'a>(&'a self, graph: ImmutableIndexDag<ErasedId>) {
        for id in graph.iter().copied() {
            let mut artifact = match self.config.cache().get_temp_artifact(id).await {
                Ok(artifact) => artifact,
                Err(error) => {
                    self.send_event(ImportError::LoadArtifact(error)).await;
                    continue;
                }
            };

            let processor = match artifact.meta.import.processor {
                Some(processor) => Some(processor),
                None => artifact
                    .meta
                    .path
                    .ext()
                    .and_then(|ext| self.config.processors().get_default(ext)),
            };

            if let Some(processor) = processor {
                let source = self.config.sources().get(artifact.path().source()).unwrap();
                let metadata = match source.read_metadata_bytes(artifact.path().path()).await {
                    Ok(metadata) => metadata,
                    Err(error) => {
                        self.send_event(ImportError::LoadMetadata(error)).await;
                        continue;
                    }
                };

                let mut ctx = ProcessContext::new(self.config.cache());
                let processor = self.config.processors()[processor];
                match processor.process(&mut ctx, artifact.data(), metadata).await {
                    Ok(data) => {
                        artifact.meta.ty = self
                            .config
                            .registry()
                            .get_ty(processor.output_asset())
                            .unwrap();
                        artifact.data = data;

                        let dependencies = ctx.dependencies.iter().map(|a| a.import.full_checksum);
                        artifact.meta.import.full_checksum = ImportMeta::get_full_checksum(
                            artifact.meta.import.checksum,
                            dependencies,
                        );
                        artifact.meta.import.dependencies = ctx
                            .dependencies
                            .iter()
                            .map(|meta| (meta.id, meta.import.full_checksum))
                            .collect();
                    }
                    Err(error) => {
                        self.send_event(ImportError::ProcessAsset(error)).await;
                        continue;
                    }
                }
            }

            if let Err(error) = self.config.cache().save_artifact(&artifact).await {
                self.send_event(ImportError::SaveAsset(error)).await;
            } else if self.states.read().await.get_load_state(id).can_reload() {
                self.reload(id);
            }
        }
    }

    async fn remove_assets<'a>(
        &'a self,
        mut paths: Vec<AssetPath<'static>>,
        library: &'a mut AssetLibrary,
    ) {
        while let Some(path) = paths.pop() {
            let Some(id) = library.remove_asset(&path) else {
                continue;
            };

            self.send_event(AssetCommand::remove(id)).await;

            let Ok(reader) = self.config.cache().get_artifact_reader(id).await else {
                let _ = self.config.cache().remove_artifact(id).await;
                continue;
            };

            let Ok(meta) = reader.into_meta().await else {
                let _ = self.config.cache().remove_artifact(id).await;
                continue;
            };

            for child in meta.children.iter().filter_map(|id| library.get_path(*id)) {
                paths.push(child.clone().into_owned());
            }

            let _ = self.config.cache().remove_artifact(id).await;
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

        Ok(Some(ImportMeta::get_checksum(&asset, &metadata)))
    }
}

#[allow(unused_imports, dead_code)]
mod tests {
    use crate::{
        asset::{Asset, DefaultSettings},
        config::{AssetConfigBuilder, importer::AssetImporter},
        database::AssetDatabase,
        io::{AssetCache, AssetIoError, AssetPath, FileSystem, SourceName, VirtualFs},
    };
    use ecs::core::task::{IoTaskPool, TaskPool};
    use serde::{Deserialize, Serialize};
    use smol::io::{AsyncReadExt, AsyncWriteExt};

    #[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
    pub struct Text(String);

    impl Asset for Text {}

    impl AssetImporter for Text {
        type Asset = Self;

        type Settings = DefaultSettings;

        type Error = AssetIoError;

        async fn import(
            _: &mut crate::config::importer::ImportContext<'_>,
            reader: &mut dyn crate::io::AsyncReader,
            _: &crate::asset::AssetMetadata<Self::Settings>,
        ) -> Result<Self::Asset, Self::Error> {
            let mut buf = String::new();
            reader
                .read_to_string(&mut buf)
                .await
                .map_err(AssetIoError::from)
                .map(|_| Text(buf))
        }

        fn extensions() -> &'static [&'static str] {
            &["txt"]
        }
    }

    #[test]
    fn test_import() {
        IoTaskPool::init(TaskPool::builder().build());

        let fs = VirtualFs::new();
        smol::block_on(async {
            let mut writer = fs.writer("text.txt".as_ref()).await.unwrap();
            writer.write(b"This is test text.").await.unwrap();
        });

        let mut config = AssetConfigBuilder::new();
        config.set_cache(AssetCache::new(VirtualFs::new()));
        config.add_source(SourceName::Default, fs);
        config.add_importer::<Text>();

        let database = AssetDatabase::init(config.build());
        database.import();

        let task = IoTaskPool::get().spawn(async move {
            std::thread::sleep(std::time::Duration::from_nanos(500)); // Simulate some delay for the import to start
            let _reader = database.writer.read().await;

            let library = database.library.read().await;
            let path = AssetPath::from_str("text.txt");
            let id = library.get_id(&path).copied().unwrap();

            let source = database.config.sources().get(&SourceName::Default).unwrap();
            let metadata = source
                .load_metadata::<<Text as AssetImporter>::Settings>("text.txt".as_ref())
                .await
                .unwrap();

            let loaded = database
                .config
                .cache()
                .load_asset::<Text>(id.into())
                .await
                .unwrap();

            assert_eq!(id, metadata.id);
            assert_eq!(&loaded.asset.0, "This is test text.");
        });

        smol::block_on(task);
    }
}
