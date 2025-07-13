use crate::{asset::ErasedId, io::path::AssetPath};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct AssetLibrary {
    ids: HashMap<ErasedId, AssetPath<'static>>,
    paths: HashMap<AssetPath<'static>, ErasedId>,
}

impl AssetLibrary {
    pub fn new() -> Self {
        Self {
            ids: HashMap::new(),
            paths: HashMap::new(),
        }
    }

    pub fn get(&self, path: &AssetPath<'static>) -> Option<&ErasedId> {
        self.paths.get(path)
    }

    pub fn path(&self, id: &ErasedId) -> Option<&AssetPath<'static>> {
        self.ids.get(id)
    }

    pub fn contains(&self, path: &AssetPath<'static>) -> bool {
        self.paths.contains_key(path)
    }

    pub fn add_asset(&mut self, id: ErasedId, path: AssetPath<'static>) {
        self.ids.insert(id, path.clone());
        self.paths.insert(path, id);
    }

    pub fn remove_asset(&mut self, path: &AssetPath<'static>) -> Option<ErasedId> {
        let id = self.paths.remove(&path)?;
        self.ids.remove(&id);
        Some(id)
    }

    pub fn ids(&self) -> &HashMap<ErasedId, AssetPath<'static>> {
        &self.ids
    }

    pub fn paths(&self) -> &HashMap<AssetPath<'static>, ErasedId> {
        &self.paths
    }

    pub fn len(&self) -> usize {
        self.ids.len()
    }

    pub fn replace(&mut self, other: AssetLibrary) {
        self.ids = other.ids;
        self.paths = other.paths;
    }
}
