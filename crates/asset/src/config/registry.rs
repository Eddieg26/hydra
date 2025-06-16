use crate::{
    asset::{Asset, AssetAction, AssetEvent, AssetType, Assets, ErasedAsset, ErasedId},
    io::{AssetIoError, deserialize},
};
use ecs::{SparseIndex, World, ext};
use serde::Deserialize;
use std::{any::TypeId, collections::HashMap, ops::Index};

#[derive(Debug)]
pub struct AssetMeta {
    pub ty: AssetType,
    pub name: &'static str,
    pub dependency_unload_action: Option<AssetAction>,

    add: fn(&mut World, ErasedId, ErasedAsset),
    remove: fn(&mut World, ErasedId) -> bool,
    modified: fn(&mut World, ErasedId),
    loaded: fn(&mut World, ErasedId),
    deserialize: Option<fn(&[u8]) -> Result<ErasedAsset, AssetIoError>>,
}

impl AssetMeta {
    pub fn new<A: Asset>(ty: AssetType) -> Self {
        Self {
            ty,
            name: ext::short_type_name::<A>(),
            dependency_unload_action: A::DEPENDENCY_UNLOAD_ACTION,
            add: |world, id, asset| {
                world.resource_mut::<Assets<A>>().insert(id, asset.into());
                world.send(AssetEvent::<A>::Added { id: id.into() });
            },
            remove: |world, id| {
                if let Some(asset) = world.resource_mut::<Assets<A>>().remove(id) {
                    world.send(AssetEvent::Removed {
                        id: id.into(),
                        asset,
                    });

                    true
                } else {
                    false
                }
            },
            modified: |world, id| {
                world.send(AssetEvent::<A>::Modified { id: id.into() });
            },
            loaded: |world, id| {
                world.send(AssetEvent::<A>::Loaded { id: id.into() });
            },
            deserialize: None,
        }
    }

    pub fn add(&self, world: &mut World, id: ErasedId, asset: ErasedAsset) {
        (self.add)(world, id, asset)
    }

    pub fn remove(&self, world: &mut World, id: ErasedId) -> bool {
        (self.remove)(world, id)
    }

    pub fn modified(&self, world: &mut World, id: ErasedId) {
        (self.modified)(world, id)
    }

    pub fn loaded(&self, world: &mut World, id: ErasedId) {
        (self.loaded)(world, id)
    }

    pub fn can_load(&self) -> bool {
        self.deserialize.is_some()
    }

    pub fn deserialize(&self, data: &[u8]) -> Option<Result<ErasedAsset, AssetIoError>> {
        self.deserialize.map(|deserialize| deserialize(data))
    }

    pub(crate) fn set_deserialize<A: Asset + for<'de> Deserialize<'de>>(&mut self) {
        self.deserialize = Some(|data: &[u8]| deserialize::<A>(data).map(ErasedAsset::new));
    }
}

#[derive(Debug)]
pub struct AssetRegistry {
    metas: Vec<AssetMeta>,
    map: HashMap<TypeId, AssetType>,
}

impl AssetRegistry {
    pub fn new() -> Self {
        Self {
            metas: Vec::new(),
            map: HashMap::new(),
        }
    }

    pub fn get_ty(&self, ty: TypeId) -> Option<AssetType> {
        self.map.get(&ty).copied()
    }

    pub fn get(&self, ty: AssetType) -> Option<&AssetMeta> {
        self.metas.get(ty.to_usize())
    }

    pub fn register<A: Asset>(&mut self) -> AssetType {
        let ty = TypeId::of::<A>();
        match self.map.get(&ty).copied() {
            Some(index) => index,
            None => {
                let index = AssetType::from_usize(self.metas.len());
                self.metas.push(AssetMeta::new::<A>(index));
                self.map.insert(ty, index);
                index
            }
        }
    }

    pub fn contains(&self, ty: TypeId) -> bool {
        self.map.contains_key(&ty)
    }

    pub(crate) fn set_deserialize<A: Asset + for<'de> Deserialize<'de>>(
        &mut self,
        index: AssetType,
    ) {
        if let Some(meta) = self.metas.get_mut(index.to_usize()) {
            meta.set_deserialize::<A>();
        }
    }
}

impl Index<AssetType> for AssetRegistry {
    type Output = AssetMeta;

    fn index(&self, index: AssetType) -> &Self::Output {
        &self.metas[index.to_usize()]
    }
}
