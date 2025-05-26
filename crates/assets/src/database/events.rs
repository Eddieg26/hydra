use super::{AssetDatabase, DatabaseEvent};
use crate::{
    asset::{AssetAction, AssetMetadata, ErasedId, Folder},
    config::{
        AssetConfig,
        importer::{ImportContext, ImportError},
        processor::ProcessContext,
    },
    io::{
        ArtifactMeta, AssetIoError, AssetLibrary, AssetPath, AssetSource, SourceName, deserialize,
    },
};
use ecs::{Command, IndexDag, core::ImmutableIndexDag};
use smol::{channel::Sender, stream::StreamExt};
use std::{
    collections::{HashMap, HashSet},
    hash::Hash,
};

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

mod tests {
    use crate::{
        asset::{Asset, DefaultSettings},
        config::{AssetConfigBuilder, importer::AssetImporter},
        database::AssetDatabase,
        io::{AssetIoError, VirtualFs},
    };
    use serde::{Deserialize, Serialize};
    use smol::io::AsyncReadExt;

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
    }

    // fn test_import() {
    //     let mut config = AssetConfigBuilder::new();
    //     config.add_source("test", VirtualFs::new(""));
    //     config.register::<Text>();
    //     config.add_importer::<Text>();

    //     let database = AssetDatabase::new(config.build());
    //     let task = database.execute(ImportAssets).unwrap();
    //     smol::block_on(task)
    // }
}
