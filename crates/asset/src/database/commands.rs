use super::AssetDatabase;
use crate::{
    AssetEvent, Assets,
    asset::{Asset, AssetId, ErasedId},
    database::state::LoadDependencies,
    ext::DeserializeExt,
    io::path::LoadPath,
};
use ecs::{Command, Commands, Events, World};
use std::{any::TypeId, collections::HashSet};

pub struct LoadAsset<A: Asset + DeserializeExt>(LoadPath<'static>, std::marker::PhantomData<A>);
impl<A: Asset + DeserializeExt, P: Into<LoadPath<'static>>> From<P> for LoadAsset<A> {
    fn from(value: P) -> Self {
        Self(value.into(), Default::default())
    }
}

impl<A: Asset + DeserializeExt> Command for LoadAsset<A> {
    fn execute(self, _: &mut World) {
        let _ = AssetDatabase::get().load::<A>(self.0);
    }
}

pub struct AddAsset<A: Asset> {
    id: AssetId<A>,
    asset: A,
    dependencies: Option<LoadDependencies>,
}

impl<A: Asset> AddAsset<A> {
    pub fn new(id: AssetId<A>, asset: A, dependencies: Option<LoadDependencies>) -> Self {
        Self {
            id,
            asset,
            dependencies,
        }
    }
}

impl<A: Asset> Command for AddAsset<A> {
    fn execute(self, world: &mut World) {
        let db = AssetDatabase::get();

        world
            .resource_mut::<Assets<A>>()
            .insert(self.id, self.asset);

        world
            .resource_mut::<Events<AssetEvent<A>>>()
            .writer()
            .send(AssetEvent::Added { id: self.id });

        if let Some(dependencies) = self.dependencies {
            let ty = db.registry().get_ty(TypeId::of::<A>()).unwrap();
            let loaded = db.states.write_blocking().loaded(
                self.id.into(),
                ty,
                &dependencies.dependencies,
                dependencies.parent,
            );

            for (id, ty) in loaded {
                let meta = db.config.registry().get(ty);
                meta.loaded(world, id);
            }
        }
    }
}

pub struct RemoveAssets(Vec<ErasedId>);
impl From<Vec<ErasedId>> for RemoveAssets {
    fn from(value: Vec<ErasedId>) -> Self {
        Self(value)
    }
}

impl Command for RemoveAssets {
    fn execute(self, world: &mut World) {
        let db = AssetDatabase::get();

        let mut states = db.states.write_blocking();
        let mut stack = self.0;
        let mut visited = HashSet::new();

        while let Some(id) = stack.pop() {
            if visited.contains(&id) {
                continue;
            }

            visited.insert(id);

            let Some(state) = states.unload(id) else {
                continue;
            };

            let meta = db.config.registry().get(state.ty());
            meta.remove(world, id);

            for dependent in state.dependents().iter().chain(state.children()).copied() {
                let Some(state) = states.get(&dependent) else {
                    continue;
                };

                if state.parent() == Some(id) {
                    stack.push(dependent);
                    continue;
                }

                db.reload(dependent);
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RemoveAsset<A: Asset>(AssetId<A>);
impl<A: Asset> From<AssetId<A>> for RemoveAsset<A> {
    fn from(value: AssetId<A>) -> Self {
        Self(value)
    }
}

impl<A: Asset> Command for RemoveAsset<A> {
    fn execute(self, world: &mut World) {
        RemoveAssets(vec![self.0.into()]).execute(world);
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
        self.add(AddAsset::new(id, asset, dependencies));
    }

    fn remove_asset<A: Asset>(&mut self, id: AssetId<A>) {
        self.add(RemoveAsset(id));
    }
}
