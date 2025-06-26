use asset::{Asset, AssetId};
use std::{
    borrow::Cow,
    hash::{Hash, Hasher},
};

pub mod binding;
pub mod buffer;
pub mod extract;
pub mod mesh;
pub mod pipeline;
pub mod shader;
pub mod texture;

pub use binding::*;
pub use buffer::*;
pub use extract::*;
pub use mesh::*;
pub use pipeline::*;
pub use shader::*;
pub use texture::*;

pub type Label = Option<Cow<'static, str>>;

pub struct AtomicId<T> {
    id: u32,
    _marker: std::marker::PhantomData<T>,
}

impl<T> AtomicId<T> {
    pub fn new() -> Self {
        use core::sync::atomic::{AtomicU32, Ordering};

        static CURRENT_ID: AtomicU32 = AtomicU32::new(0);

        let id = CURRENT_ID.fetch_add(1, Ordering::Relaxed);
        Self {
            id,
            _marker: std::marker::PhantomData,
        }
    }

    pub fn id(&self) -> u32 {
        self.id
    }
}

impl<T> From<AtomicId<T>> for u32 {
    fn from(value: AtomicId<T>) -> Self {
        value.id
    }
}

impl<T> From<u32> for AtomicId<T> {
    fn from(value: u32) -> Self {
        Self {
            id: value,
            _marker: std::marker::PhantomData,
        }
    }
}

impl<T> Clone for AtomicId<T> {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            _marker: std::marker::PhantomData,
        }
    }
}

impl<T> std::fmt::Debug for AtomicId<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:x}", self.id)
    }
}

impl<T> Copy for AtomicId<T> {}

impl<T> PartialEq for AtomicId<T> {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl<T> Eq for AtomicId<T> {}

impl<T> Hash for AtomicId<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state)
    }
}

impl<T> PartialOrd for AtomicId<T> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.id.partial_cmp(&other.id)
    }
}

impl<T> Ord for AtomicId<T> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.id.cmp(&other.id)
    }
}

pub trait OptionalAssetId<A: Asset> {
    fn into_optional_id(self) -> Option<AssetId<A>>;
}

impl<A: Asset, S: AsRef<AssetId<A>>> OptionalAssetId<A> for S {
    fn into_optional_id(self) -> Option<AssetId<A>> {
        Some(*self.as_ref())
    }
}
