use super::AssetDatabase;
use crate::asset::{AssetAction, ErasedId};
use ecs::Command;
use std::collections::HashSet;

pub struct UnloadAsset(ErasedId);

impl Command for UnloadAsset {
    fn execute(self, world: &mut ecs::World) {
        UnloadAssets(vec![self.0]).execute(world);
    }
}

pub struct UnloadAssets(Vec<ErasedId>);

impl Command for UnloadAssets {
    fn execute(self, world: &mut ecs::World) {
        let database = world.resource::<AssetDatabase>().clone();
        let mut states = database.states.write_blocking();

        let mut stack = self.0;
        let mut unloaded = HashSet::new();
        let mut reload = HashSet::new();

        while let Some(id) = stack.pop() {
            if unloaded.contains(&id) || reload.contains(&id) {
                continue;
            }

            unloaded.insert(id);

            let Some(state) = states.unload(id) else {
                continue;
            };

            let meta = database.config.registry().get(state.ty()).unwrap();
            meta.remove(world, id);

            for dependent in state.dependents() {
                let Some(state) = states.get(*dependent) else {
                    continue;
                };

                match state.unload_action() {
                    Some(AssetAction::Unload) => stack.push(*dependent),
                    Some(AssetAction::Reload) => {
                        reload.insert(*dependent);
                    }
                    None => continue,
                }
            }
        }
    }
}

#[allow(unused_imports, dead_code)]
mod tests {
    use crate::{
        asset::{Asset, DefaultSettings},
        config::{AssetConfigBuilder, importer::AssetImporter},
        database::AssetDatabase,
        io::{AssetIoError, AssetPath, FileSystem, VirtualFs},
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

        let mut config = AssetConfigBuilder::new();
        config.register::<Text>();
        config.add_importer::<Text>();

        let fs = VirtualFs::new();
        smol::block_on(async {
            let mut writer = fs.writer("text.txt".as_ref()).await.unwrap();
            writer.write(b"This is test text.").await.unwrap();
        });

        config.add_source("test", fs);
        config.set_cache(VirtualFs::new());

        let database = AssetDatabase::new(config.build());
        database.import();

        std::thread::sleep(std::time::Duration::from_nanos(500));

        let task = {
            IoTaskPool::get().spawn(async move {
                let _reader = database.writer.read().await;

                let library = database.library.read().await;
                let path = AssetPath::from_str("test://text.txt");
                let id = library.get_id(&path).copied().unwrap();

                let source = database.config.sources().get(&"test".into()).unwrap();
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
            })
        };

        smol::block_on(task);
    }
}
