use ecs::{Event, Resource};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fmt::{Debug, Display},
    hash::Hash,
    marker::PhantomData,
};
use uuid::Uuid;

pub trait Asset: AssetDependencies + Send + Sync + 'static {}

pub trait AssetDependencies {
    fn get_dependencies(&self, _: impl FnMut(ErasedId)) {}
}

pub struct AssetId<A: Asset>(Uuid, PhantomData<A>);

impl<A: Asset> AssetId<A> {
    pub fn new() -> Self {
        Self(Uuid::new_v4(), Default::default())
    }

    pub const fn from_u128(value: u128) -> Self {
        Self(Uuid::from_u128(value), PhantomData)
    }

    pub fn with_namespace(&self, name: &[u8]) -> Self {
        Self(Uuid::new_v5(&self.0, name), Default::default())
    }
}

impl<A: Asset> AsRef<Uuid> for AssetId<A> {
    fn as_ref(&self) -> &Uuid {
        &self.0
    }
}

impl<A: Asset, S: Asset> AsRef<AssetId<S>> for AssetId<A> {
    fn as_ref(&self) -> &AssetId<S> {
        unsafe { std::mem::transmute(self) }
    }
}

impl<A: Asset> From<Uuid> for AssetId<A> {
    fn from(value: Uuid) -> Self {
        Self(value, Default::default())
    }
}

impl<A: Asset> Into<Uuid> for AssetId<A> {
    fn into(self) -> Uuid {
        self.0
    }
}

impl<A: Asset> Debug for AssetId<A> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0.to_string())
    }
}

impl<A: Asset> Display for AssetId<A> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0.to_string())
    }
}

impl<A: Asset> Copy for AssetId<A> {}
impl<A: Asset> Clone for AssetId<A> {
    fn clone(&self) -> Self {
        Self(self.0.clone(), self.1.clone())
    }
}

impl<A: Asset> Eq for AssetId<A> {}
impl<A: Asset> PartialEq for AssetId<A> {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl<A: Asset> Ord for AssetId<A> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.cmp(&other.0)
    }
}

impl<A: Asset> PartialOrd for AssetId<A> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.0.partial_cmp(&other.0)
    }
}

impl<A: Asset> Serialize for AssetId<A> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.0.serialize(serializer)
    }
}

impl<A: Asset> Hash for AssetId<A> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

impl<'de, A: Asset> Deserialize<'de> for AssetId<A> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let id = Uuid::deserialize(deserializer)?;
        Ok(Self::from(id))
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ErasedId(Uuid);

impl ErasedId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    pub const fn from_u128(value: u128) -> Self {
        Self(Uuid::from_u128(value))
    }

    pub fn with_namespace(&self, name: &[u8]) -> Self {
        Self(Uuid::new_v5(&self.0, name))
    }
}

impl std::fmt::Debug for ErasedId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0.to_string())
    }
}

impl std::fmt::Display for ErasedId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(&self, f)
    }
}

impl AsRef<Uuid> for ErasedId {
    fn as_ref(&self) -> &Uuid {
        &self.0
    }
}

impl From<Uuid> for ErasedId {
    fn from(value: Uuid) -> Self {
        Self(value)
    }
}

impl Into<Uuid> for ErasedId {
    fn into(self) -> Uuid {
        self.0
    }
}

impl<A: Asset> From<AssetId<A>> for ErasedId {
    fn from(value: AssetId<A>) -> Self {
        Self(value.0)
    }
}

impl<A: Asset> From<ErasedId> for AssetId<A> {
    fn from(value: ErasedId) -> Self {
        AssetId::from(value.0)
    }
}

#[derive(Resource)]
pub struct Assets<A: Asset>(HashMap<AssetId<A>, A>);
impl<A: Asset> Assets<A> {
    pub fn new() -> Self {
        Self(HashMap::new())
    }
}

impl<A: Asset> std::ops::Deref for Assets<A> {
    type Target = HashMap<AssetId<A>, A>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<A: Asset> std::ops::DerefMut for Assets<A> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AssetType(pub(crate) u32);
impl AssetType {
    pub const UNKNOWN: Self = Self(u32::MAX);
}

impl AssetDependencies for ErasedId {
    fn get_dependencies(&self, mut f: impl FnMut(ErasedId)) {
        f(*self);
    }
}

impl AssetDependencies for Option<ErasedId> {
    fn get_dependencies(&self, mut f: impl FnMut(ErasedId)) {
        if let Some(id) = *self {
            f(id);
        }
    }
}

impl<A: Asset> AssetDependencies for AssetId<A> {
    fn get_dependencies(&self, mut f: impl FnMut(ErasedId)) {
        f((*self).into());
    }
}

impl<A: Asset> AssetDependencies for Option<AssetId<A>> {
    fn get_dependencies(&self, mut f: impl FnMut(ErasedId)) {
        if let Some(id) = *self {
            f(id.into());
        }
    }
}

impl AssetDependencies for Vec<ErasedId> {
    fn get_dependencies(&self, mut f: impl FnMut(ErasedId)) {
        for id in self {
            f(*id);
        }
    }
}

impl AssetDependencies for Vec<Option<ErasedId>> {
    fn get_dependencies(&self, mut f: impl FnMut(ErasedId)) {
        for id in self.iter().filter_map(|i| *i) {
            f(id);
        }
    }
}

impl<A: Asset> AssetDependencies for Vec<AssetId<A>> {
    fn get_dependencies(&self, mut f: impl FnMut(ErasedId)) {
        for id in self {
            f((*id).into());
        }
    }
}

impl<A: Asset> AssetDependencies for Vec<Option<AssetId<A>>> {
    fn get_dependencies(&self, mut f: impl FnMut(ErasedId)) {
        for id in self.iter().filter_map(|i| *i) {
            f(id.into());
        }
    }
}

#[derive(Event)]
/// Event representing changes to an [`Asset`] in the [`AssetDatabase`].
pub enum AssetEvent<A: Asset> {
    /// An asset was added, modified, removed, or loaded.
    Added { id: AssetId<A> },
    /// An asset was modified.
    Modified { id: AssetId<A> },
    /// An asset was removed.
    Removed { id: AssetId<A>, asset: A },
    /// An asset and its dependencies were loaded.
    Loaded { id: AssetId<A> },
}

impl<A: Asset> std::fmt::Debug for AssetEvent<A> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AssetEvent::Added { id } => f.write_fmt(format_args!("AssetEvent::Added({:?})", id)),
            AssetEvent::Modified { id } => {
                f.write_fmt(format_args!("AssetEvent::Modified({:?})", id))
            }
            AssetEvent::Removed { id, .. } => {
                f.write_fmt(format_args!("AssetEvent::Removed({:?})", id))
            }
            AssetEvent::Loaded { id } => f.write_fmt(format_args!("AssetEvent::Loaded({:?})", id)),
        }
    }
}

impl<A: Asset> AssetEvent<A> {
    pub fn id(&self) -> AssetId<A> {
        match self {
            AssetEvent::Added { id } => *id,
            AssetEvent::Modified { id } => *id,
            AssetEvent::Removed { id, .. } => *id,
            AssetEvent::Loaded { id } => *id,
        }
    }
}

pub struct ErasedAsset {
    ptr: *const u8,
    size: usize,
    drop: fn(*const u8),
}

impl ErasedAsset {
    pub unsafe fn into<A: Asset>(self) -> A {
        // Read from heap and reconstruct box to deallocate properly
        let asset = unsafe { Box::from_raw(self.ptr as *mut A) };
        std::mem::forget(self); // Prevent our drop from running
        *asset
    }

    pub fn size(&self) -> usize {
        self.size
    }
}

impl<A: Asset> From<A> for ErasedAsset {
    fn from(asset: A) -> Self {
        let ptr = Box::into_raw(Box::new(asset)) as *const u8;
        let size = std::mem::size_of::<A>();

        let drop = |ptr: *const u8| unsafe {
            let asset = Box::from_raw(ptr as *mut A);
            std::mem::drop(asset);
        };

        Self { ptr, size, drop }
    }
}

unsafe impl Send for ErasedAsset {}
unsafe impl Sync for ErasedAsset {}

impl Drop for ErasedAsset {
    fn drop(&mut self) {
        (self.drop)(self.ptr);
    }
}
