use super::{AssetIoError, deserialize, serialize};
use crate::asset::{Asset, AssetId, AssetType, ErasedId};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, path::PathBuf};

pub struct AssetLibrary {
    id_map: HashMap<PathBuf, ErasedId>,
    path_map: HashMap<ErasedId, PathBuf>,
}

impl AssetLibrary {
    pub fn new() -> Self {
        Self {
            id_map: HashMap::new(),
            path_map: HashMap::new(),
        }
    }

    pub fn get_id(&self, path: &PathBuf) -> Option<&ErasedId> {
        self.id_map.get(path)
    }

    pub fn get_path(&self, id: ErasedId) -> Option<&PathBuf> {
        self.path_map.get(&id)
    }

    pub fn add_id(&mut self, path: PathBuf, id: ErasedId) {
        self.id_map.insert(path.clone(), id);
        self.path_map.insert(id, path);
    }

    pub fn remove_id(&mut self, path: &PathBuf) -> Option<ErasedId> {
        self.id_map.remove(path).and_then(|id| {
            self.path_map.remove(&id);
            Some(id)
        })
    }

    pub fn len(&self) -> usize {
        self.id_map.len()
    }

    pub fn clear(&mut self) {
        self.id_map.clear();
        self.path_map.clear();
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetChecksum {
    pub value: u32,
    pub full: u32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ArtifactHeader {
    pub asset: u32,
    pub meta: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactMeta {
    pub id: ErasedId,
    pub ty: AssetType,
    pub checksum: AssetChecksum,
    pub dependencies: Vec<ErasedId>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Artifact {
    header: ArtifactHeader,
    meta: ArtifactMeta,
    data: Vec<u8>,
}

impl Artifact {
    pub fn new<A: Asset + Serialize>(asset: &A, meta: ArtifactMeta) -> Result<Self, AssetIoError> {
        let data = serialize(&asset)?;

        let header = ArtifactHeader {
            asset: data.len() as u32,
            meta: serialize(&meta)?.len() as u32,
        };

        Ok(Self { header, meta, data })
    }

    pub fn from_raw_parts(header: ArtifactHeader, meta: ArtifactMeta, data: Vec<u8>) -> Self {
        Self { header, meta, data }
    }

    pub fn header(&self) -> ArtifactHeader {
        self.header
    }

    pub fn meta(&self) -> &ArtifactMeta {
        &self.meta
    }

    pub fn data(&self) -> &[u8] {
        &self.data
    }

    pub fn asset<A: Asset + for<'a> Deserialize<'a>>(&self) -> Result<A, AssetIoError> {
        deserialize(&self.data)
    }
}

pub struct LoadedAsset<A: Asset> {
    id: AssetId<A>,
    data: Vec<u8>,
}

pub struct LoadedAssets<A: Asset> {
    assets: Vec<LoadedAsset<A>>,
    _marker: std::marker::PhantomData<A>,
}
