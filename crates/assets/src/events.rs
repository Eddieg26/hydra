use crate::asset::{Asset, AssetId, Assets, ErasedId};
use ecs::Command;

pub struct LoadAsset<A: Asset> {
    pub id: AssetId<A>,
    pub asset: A,
    pub dependencies: Vec<ErasedId>,
}

impl<A: Asset> LoadAsset<A> {
    pub fn new(id: impl Into<AssetId<A>>, asset: A) -> Self {
        Self {
            id: id.into(),
            asset,
            dependencies: vec![],
        }
    }

    pub fn with_dependencies(mut self, dependencies: Vec<ErasedId>) -> Self {
        self.dependencies = dependencies;
        self
    }
}

impl<A: Asset> Command for LoadAsset<A> {
    fn execute(self, world: &mut ecs::World) {
        let assets = world.resource_mut::<Assets<A>>();
        assets.insert(self.id, self.asset);
    }
}

pub struct UnloadAsset<A: Asset>(AssetId<A>);
impl<A: Asset> UnloadAsset<A> {
    pub fn new(id: impl Into<AssetId<A>>) -> Self {
        Self(id.into())
    }
}

impl<A: Asset> Command for UnloadAsset<A> {
    fn execute(self, world: &mut ecs::World) {
        let assets = world.resource_mut::<Assets<A>>();
        let asset = assets.remove(self.0);
    }
}
