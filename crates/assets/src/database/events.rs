use super::AssetDatabase;
use crate::{
    asset::{Asset, AssetAction, AssetId, ErasedAsset, ErasedId},
    io::ArtifactMeta,
};
use ecs::Command;
use std::collections::HashSet;

pub struct AssetLoaded {
    pub asset: ErasedAsset,
    pub meta: ArtifactMeta,
}

impl Command for AssetLoaded {
    fn execute(self, world: &mut ecs::World) {
        let database = AssetDatabase::get();

        if let Some(meta) = database.config.registry().get(self.meta.ty) {
            meta.add(world, self.meta.id, self.asset);
        };

        let mut states = database.states.write_blocking();

        let mut stack = vec![self.meta.id];

        while let Some(id) = stack.pop() {
            states.loaded(&self.meta);
        }
    }
}

pub struct UnloadAsset(pub ErasedId);

impl Command for UnloadAsset {
    fn execute(self, world: &mut ecs::World) {
        UnloadAssets(vec![self.0]).execute(world);
    }
}

pub struct UnloadAssets(Vec<ErasedId>);

impl Command for UnloadAssets {
    fn execute(self, world: &mut ecs::World) {
        let database = AssetDatabase::get();
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

                if state.parent() == Some(id) {
                    stack.push(*dependent);
                    continue;
                }

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

/// Event representing changes to an [`Asset`] in the [`AssetDatabase`].
pub enum AssetEvent<A: Asset> {
    /// An asset was added, modified, removed, or loaded.
    Added { id: AssetId<A> },
    /// An asset was modified.
    Modified { id: AssetId<A> },
    /// An asset was removed.
    Removed { id: AssetId<A> },
    /// An asset and its dependencies were loaded.
    Loaded { id: AssetId<A> },
}
