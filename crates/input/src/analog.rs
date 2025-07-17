use std::{collections::HashMap, hash::Hash};
use ecs::Resource;

#[derive(Resource)]
pub struct Analogs<T: Hash + Copy + Eq + 'static>(HashMap<T, f32>);

impl<T: Hash + Copy + Eq + std::fmt::Debug> std::fmt::Debug for Analogs<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Analogs").field("analogs", &self.0).finish()
    }
}

impl<T: Hash + Copy + Eq> Analogs<T> {
    pub const MIN: f32 = -1.0;

    pub const MAX: f32 = 1.0;

    pub fn new() -> Self {
        Self(HashMap::new())
    }

    pub fn set(&mut self, key: T, value: f32) {
        self.0.insert(key, value);
    }

    pub fn get(&self, key: &T) -> Option<f32> {
        self.0.get(key).map(|v| v.clamp(Self::MIN, Self::MAX))
    }

    pub fn get_unclamed(&self, key: &T) -> Option<f32> {
        self.0.get(key).copied()
    }

    pub fn remove(&mut self, key: &T) {
        self.0.remove(key);
    }

    pub fn iter(&self) -> impl Iterator<Item = (&T, &f32)> {
        self.0.iter()
    }

    pub fn analogs(&self) -> impl Iterator<Item = &T> {
        self.0.keys()
    }
}
