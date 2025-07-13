use crate::{
    AssetEvent, Assets, ErasedId,
    asset::{Asset, AssetType, ErasedAsset},
    ext::DeserializeExt,
};
use ecs::World;
use std::{any::TypeId, collections::HashMap};

#[derive(Debug)]
pub struct AssetMeta {
    pub ty: AssetType,
    pub name: &'static str,

    add: fn(&mut World, ErasedId, ErasedAsset),
    remove: fn(&mut World, ErasedId) -> bool,
    modified: fn(&mut World, ErasedId),
    loaded: fn(&mut World, ErasedId),
    deserialize: Option<fn(&[u8]) -> Result<ErasedAsset, bincode::error::DecodeError>>,
}

impl AssetMeta {
    pub fn new<A: Asset>(ty: AssetType) -> Self {
        Self {
            ty,
            name: std::any::type_name::<A>(),
            add: |world, id, asset| {
                world
                    .resource_mut::<Assets<A>>()
                    .insert(id.into(), unsafe { asset.into() });
                world.send(AssetEvent::<A>::Added { id: id.into() });
            },
            remove: |world, id| {
                if let Some(asset) = world.resource_mut::<Assets<A>>().remove(&id.into()) {
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

    pub fn deserialize(
        &self,
        data: &[u8],
    ) -> Option<Result<ErasedAsset, bincode::error::DecodeError>> {
        self.deserialize.map(|f| f(data))
    }

    pub fn set_deserialize<A: Asset + DeserializeExt>(&mut self) {
        let f = |data: &[u8]| {
            let asset = A::from_bytes(data)?;
            Ok(ErasedAsset::from(asset))
        };

        self.deserialize = Some(f);
    }
}

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

    pub fn register<A: Asset>(&mut self) -> AssetType {
        let type_id = TypeId::of::<A>();
        if let Some(ty) = self.map.get(&type_id).copied() {
            ty
        } else {
            let ty = AssetType(self.metas.len() as u32);
            self.metas.push(AssetMeta::new::<A>(ty));
            self.map.insert(type_id, ty);

            ty
        }
    }

    pub fn set_deserialize<A: Asset + DeserializeExt>(&mut self) {
        let ty = self.register::<A>();
        self.metas[ty.0 as usize].set_deserialize::<A>();
    }

    pub fn get(&self, ty: AssetType) -> &AssetMeta {
        &self.metas[ty.0 as usize]
    }

    pub fn get_ty(&self, ty: TypeId) -> Option<AssetType> {
        self.map.get(&ty).copied()
    }

    pub fn is_registered<A: Asset>(&self) -> bool {
        self.map.contains_key(&TypeId::of::<A>())
    }
}
