use super::AssetDatabase;
use crate::asset::{Asset, AssetAction, AssetId, AssetType, ErasedAsset, ErasedId};
use ecs::{Command, Commands, World};
use std::{any::TypeId, collections::HashSet};

pub struct LoadDependencies {
    pub parent: Option<ErasedId>,
    pub dependencies: Vec<ErasedId>,
}

impl LoadDependencies {
    pub fn new(parent: Option<ErasedId>, dependencies: impl IntoIterator<Item = ErasedId>) -> Self {
        Self {
            parent,
            dependencies: dependencies.into_iter().collect(),
        }
    }
}

pub enum AssetCommand {
    Add {
        id: ErasedId,
        ty: AssetType,
        asset: ErasedAsset,
        dependencies: Option<LoadDependencies>,
    },
    Modify {
        id: ErasedId,
        ty: AssetType,
    },
    Remove {
        id: ErasedId,
    },
    Loaded {
        id: ErasedId,
        ty: AssetType,
    },
}

impl Command for AssetCommand {
    fn execute(self, world: &mut ecs::World) {
        use AssetCommand::{Add, Loaded, Modify, Remove};

        let db = AssetDatabase::get();

        match self {
            Add {
                id,
                ty,
                asset,
                dependencies,
            } => {
                let meta = db.config.registry().get(ty).unwrap();
                meta.add(world, id, asset);

                if let Some(dependencies) = dependencies {
                    let loaded = db.states.write_blocking().loaded(
                        id,
                        ty,
                        &dependencies.dependencies,
                        dependencies.parent,
                        meta.dependency_unload_action,
                    );

                    for (id, ty) in loaded {
                        let meta = db.config.registry().get(ty).unwrap();
                        meta.loaded(world, id);
                    }
                }
            }
            Modify { id, ty } => {
                let meta = db.config.registry().get(ty).unwrap();
                meta.modified(world, id);
            }
            Remove { id } => Self::remove_asset(world, db, vec![id]),
            Loaded { id, ty } => {
                let meta = db.config.registry().get(ty).unwrap();
                meta.loaded(world, id);
            }
        }
    }
}

impl AssetCommand {
    pub fn add(
        id: impl Into<ErasedId>,
        ty: AssetType,
        asset: impl Into<ErasedAsset>,
        dependencies: impl Into<Option<LoadDependencies>>,
    ) -> Self {
        Self::Add {
            id: id.into(),
            ty,
            asset: asset.into(),
            dependencies: dependencies.into(),
        }
    }

    pub fn modify(id: impl Into<ErasedId>, ty: AssetType) -> Self {
        Self::Modify { id: id.into(), ty }
    }

    pub fn remove(id: impl Into<ErasedId>) -> Self {
        Self::Remove { id: id.into() }
    }

    pub fn loaded(id: impl Into<ErasedId>, ty: AssetType) -> Self {
        Self::Loaded { id: id.into(), ty }
    }

    fn remove_asset(world: &mut World, database: &AssetDatabase, ids: Vec<ErasedId>) {
        let mut states = database.states.write_blocking();

        let mut stack = ids;
        let mut visited = HashSet::new();

        while let Some(id) = stack.pop() {
            if visited.contains(&id) {
                continue;
            }

            visited.insert(id);

            let Some(state) = states.unload(id) else {
                continue;
            };

            let meta = database.config.registry().get(state.ty()).unwrap();
            meta.remove(world, id);

            for dependent in state.dependents().iter().chain(state.children()).copied() {
                let Some(state) = states.get(&dependent) else {
                    continue;
                };

                if state.parent() == Some(id) {
                    stack.push(dependent);
                    continue;
                }

                match state.unload_action() {
                    Some(AssetAction::Unload) => stack.push(dependent),
                    Some(AssetAction::Reload) => database.reload(dependent),
                    None => continue,
                }
            }
        }
    }
}

pub trait AssetDatabaseCommands {
    fn add_asset<A: Asset>(
        &mut self,
        id: AssetId<A>,
        asset: A,
        dependencies: Option<LoadDependencies>,
    );
    fn remove_asset<A: Asset>(&mut self, id: AssetId<A>);
}

impl AssetDatabaseCommands for Commands<'_, '_> {
    fn add_asset<A: Asset>(
        &mut self,
        id: AssetId<A>,
        asset: A,
        dependencies: Option<LoadDependencies>,
    ) {
        let database = AssetDatabase::get();
        let ty = database
            .config
            .registry()
            .get_ty(TypeId::of::<A>())
            .unwrap();
        self.add(AssetCommand::add(id, ty, asset, dependencies));
    }

    fn remove_asset<A: Asset>(&mut self, id: AssetId<A>) {
        self.add(AssetCommand::remove(id));
    }
}
