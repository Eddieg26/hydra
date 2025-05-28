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
