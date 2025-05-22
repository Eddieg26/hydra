use crate::asset::{Asset, AssetAction, AssetType};
use ecs::{SparseIndex, ext};
use std::{any::TypeId, collections::HashMap, ops::Index};

pub struct AssetMeta {
    pub name: &'static str,
    pub dependency_unload_action: Option<AssetAction>,
}

impl AssetMeta {
    pub fn new<A: Asset>() -> Self {
        Self {
            name: ext::short_type_name::<A>(),
            dependency_unload_action: A::DEPENDENCY_UNLOAD_ACTION,
        }
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
                self.metas.push(AssetMeta::new::<A>());
                self.map.insert(ty, index);
                index
            }
        }
    }
}

impl Index<AssetType> for AssetRegistry {
    type Output = AssetMeta;

    fn index(&self, index: AssetType) -> &Self::Output {
        &self.metas[index.to_usize()]
    }
}
