use super::{AssetDatabase, DatabaseEvent};
use crate::{
    asset::{AssetMetadata, ErasedId, Folder},
    config::{
        AssetConfig,
        importer::{self, ImportContext, ImportError},
        processor::ProcessContext,
    },
    io::{AssetIoError, AssetLibrary, AssetPath, AssetSource, PathExt, SourceName, deserialize},
};
use ecs::{
    IndexDag,
    core::{ImmutableIndexDag, task::IoTaskPool},
};
use smol::{channel::Sender, stream::StreamExt};
use std::collections::HashMap;

#[derive(Default)]
pub struct ImportInfo {
    imported: Vec<AssetPath<'static>>,
    removed: Vec<AssetPath<'static>>,
}

impl AssetDatabase {
    pub fn import(&self) {
        let db = self.clone();

        let import = async move {
            let AssetDatabase {
                config,
                library,
                sender,
                writer,
                ..
            } = &db;

            let _writer = writer.write().await;

            if let Err(error) = config.cache().create_temp().await {
                let _ = sender.send(DatabaseEvent::ImportError(ImportError::Unknown(error)));
                return;
            }

            for _ in 0..3 {
                let mut library = library.write().await;
                let mut imported = vec![];
                let mut removed = vec![];

                for (name, source) in config.sources().iter() {
                    match db
                        .import_source(name, source, config, &library, sender)
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

                db.remove_assets(removed, config, &mut library).await;

                let process_list = db
                    .import_assets(imported, config, &mut library, sender)
                    .await;

                db.process_assets(process_list, config, &sender).await;
            }

            let _ = config.cache().delete_temp().await;
        };

        IoTaskPool::get().spawn(import).detach();
    }

    async fn import_source<'a>(
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

        let mut import = vec![];

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
                        Ok(Some(path)) => import.push(path.into_static()),
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
        config: &'a AssetConfig,
        library: &'a AssetLibrary,
    ) -> Result<Option<AssetPath<'a>>, AssetIoError> {
        let Some(id) = library.get_id(&path).copied() else {
            return Ok(Some(path));
        };

        let meta_path = path.path().append_ext("meta");
        if !source.exists(&meta_path).await? {
            return Ok(Some(path));
        }

        let Ok(mut reader) = config.cache().get_artifact_reader(id).await else {
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

        let full_checksum = importer::get_full_checksum(checksum, dependencies);
        if full_checksum != meta.import.full_checksum {
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
                        artifact.data = data;

                        let dependencies = ctx.dependencies.iter().map(|a| a.import.full_checksum);
                        artifact.meta.import.full_checksum = importer::get_full_checksum(
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
                        let _ = events.send(ImportError::ProcessAsset(error).into()).await;
                        continue;
                    }
                }
            }

            if let Err(error) = config.cache().save_artifact(&artifact).await {
                let _ = events.send(ImportError::SaveAsset(error).into()).await;
            }
        }
    }

    async fn remove_assets<'a>(
        &'a self,
        paths: Vec<AssetPath<'static>>,
        config: &'a AssetConfig,
        library: &'a mut AssetLibrary,
    ) {
        let mut removed = vec![];
        for path in paths {
            let Some(id) = library.remove_asset(&path) else {
                continue;
            };

            removed.push(id);

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

        Ok(Some(importer::get_checksum(&asset, &metadata)))
    }
}
