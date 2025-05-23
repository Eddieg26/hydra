use crate::io::source::AssetPath;
use ecs::{Resource, SparseIndex};
use serde::{Deserialize, Serialize, de::Visitor, ser::SerializeStruct};
use std::{
    collections::{HashMap, HashSet},
    hash::Hash,
    path::PathBuf,
};

pub trait AssetDependencies: Send + Sync {
    fn add(&mut self, id: impl Into<ErasedId>);
}

impl AssetDependencies for Vec<ErasedId> {
    fn add(&mut self, id: impl Into<ErasedId>) {
        self.push(id.into());
    }
}

impl<A: Asset> AssetDependencies for Vec<AssetId<A>> {
    fn add(&mut self, id: impl Into<ErasedId>) {
        self.push(AssetId::<A>::from(id.into()));
    }
}

impl AssetDependencies for HashSet<ErasedId> {
    fn add(&mut self, id: impl Into<ErasedId>) {
        self.insert(id.into());
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssetAction {
    Reload,
    Unload,
}

pub trait Asset: Send + Sync + 'static {
    const DEPENDENCY_UNLOAD_ACTION: Option<AssetAction> = None;

    fn dependencies(&self, _dependencies: &mut impl AssetDependencies) {}
}

pub enum AssetDependency {
    Import(ErasedId),
    Load(ErasedId),
}

pub trait Settings: Default + Send + Sync + 'static {}

#[derive(Clone, Serialize, Deserialize)]
pub struct DefaultSettings {
    pub created: u64,
}

impl Default for DefaultSettings {
    fn default() -> Self {
        let created = match std::time::SystemTime::now().elapsed() {
            Ok(elapsed) => elapsed.as_secs(),
            Err(_) => 0,
        };

        Self { created }
    }
}

impl Settings for DefaultSettings {}

pub struct AssetId<A: Asset>(uuid::Uuid, std::marker::PhantomData<A>);
impl<A: Asset> AssetId<A> {
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4(), Default::default())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ErasedId(uuid::Uuid);
impl ErasedId {
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4())
    }
}

impl ToString for ErasedId {
    fn to_string(&self) -> String {
        self.0.to_string()
    }
}

impl Serialize for ErasedId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.0.as_u128().serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for ErasedId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = u128::deserialize(deserializer)?;
        Ok(Self(uuid::Uuid::from_u128(value)))
    }
}

impl<A: Asset> Serialize for AssetId<A> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.0.as_u128().serialize(serializer)
    }
}

impl<'de, A: Asset> Deserialize<'de> for AssetId<A> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = u128::deserialize(deserializer)?;
        Ok(Self(uuid::Uuid::from_u128(value), Default::default()))
    }
}

impl<A: Asset> std::fmt::Debug for AssetId<A> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("AssetId")
            .field(&self.0)
            .field(&self.1)
            .finish()
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
        self.0 == other.0 && self.1 == other.1
    }
}

impl<A: Asset> Hash for AssetId<A> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

impl<A: Asset> From<ErasedId> for AssetId<A> {
    fn from(value: ErasedId) -> Self {
        Self(value.0, Default::default())
    }
}

impl<A: Asset> From<uuid::Uuid> for AssetId<A> {
    fn from(value: uuid::Uuid) -> Self {
        Self(value, Default::default())
    }
}

impl<A: Asset> From<AssetId<A>> for ErasedId {
    fn from(value: AssetId<A>) -> Self {
        ErasedId(value.0)
    }
}

impl From<uuid::Uuid> for ErasedId {
    fn from(value: uuid::Uuid) -> Self {
        Self(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AssetType(u32);
impl AssetType {
    pub const NONE: AssetType = AssetType(0);
}

impl SparseIndex for AssetType {
    fn to_usize(self) -> usize {
        self.0 as usize
    }

    fn from_usize(index: usize) -> Self {
        Self(index as u32)
    }
}

pub struct AssetMetadata<S: Settings> {
    pub id: ErasedId,
    settings: S,
}

impl<S: Settings> AssetMetadata<S> {
    pub fn new(id: impl Into<ErasedId>, settings: S) -> Self {
        Self {
            id: id.into(),
            settings,
        }
    }
}

impl<S: Settings> Default for AssetMetadata<S> {
    fn default() -> Self {
        Self {
            id: ErasedId::new(),
            settings: Default::default(),
        }
    }
}

impl<S: Settings> std::ops::Deref for AssetMetadata<S> {
    type Target = S;

    fn deref(&self) -> &Self::Target {
        &self.settings
    }
}

impl<S: Settings> std::ops::DerefMut for AssetMetadata<S> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.settings
    }
}

impl<T: Settings + Serialize> Serialize for AssetMetadata<T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut metadata = serializer.serialize_struct("AssetMetadata", 2)?;
        metadata.serialize_field("id", &self.id)?;
        metadata.serialize_field("settings", &self.settings)?;
        metadata.end()
    }
}

impl<'de, T: Settings + Deserialize<'de>> Deserialize<'de> for AssetMetadata<T> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct MetadataVisitor<S: Settings>(std::marker::PhantomData<S>);
        impl<'v, S: Settings + Deserialize<'v>> Visitor<'v> for MetadataVisitor<S> {
            type Value = AssetMetadata<S>;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("struct AssetMetadata")
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::MapAccess<'v>,
            {
                let id = map.next_value()?;
                let settings = map.next_value()?;

                Ok(AssetMetadata { id, settings })
            }
        }

        deserializer.deserialize_struct(
            "AssetMetadata",
            &["id", "settings"],
            MetadataVisitor::<T>(Default::default()),
        )
    }
}

pub struct Assets<A: Asset> {
    assets: HashMap<AssetId<A>, A>,
}

impl<A: Asset> Assets<A> {
    pub fn new() -> Self {
        Self {
            assets: HashMap::new(),
        }
    }

    pub fn get(&self, id: impl Into<AssetId<A>>) -> Option<&A> {
        self.assets.get(&id.into())
    }

    pub fn get_mut(&mut self, id: impl Into<AssetId<A>>) -> Option<&mut A> {
        self.assets.get_mut(&id.into())
    }

    pub fn insert(&mut self, id: impl Into<AssetId<A>>, asset: A) -> Option<A> {
        self.assets.insert(id.into(), asset)
    }

    pub fn remove(&mut self, id: impl Into<AssetId<A>>) -> Option<A> {
        self.assets.remove(&id.into())
    }

    pub fn iter(&self) -> impl Iterator<Item = (&AssetId<A>, &A)> {
        self.assets.iter()
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = (&AssetId<A>, &mut A)> {
        self.assets.iter_mut()
    }

    pub fn values(&self) -> impl Iterator<Item = &A> {
        self.assets.values()
    }

    pub fn values_mut(&mut self) -> impl Iterator<Item = &mut A> {
        self.assets.values_mut()
    }

    pub fn contains(&self, id: impl Into<AssetId<A>>) -> bool {
        self.assets.contains_key(&id.into())
    }

    pub fn len(&mut self) -> usize {
        self.assets.len()
    }

    pub fn clear(&mut self) {
        self.assets.clear();
    }
}

impl<A: Asset> Resource for Assets<A> {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LoadPath<'a> {
    Id(ErasedId),
    Path(AssetPath<'a>),
}

impl<I: Into<ErasedId>> From<I> for LoadPath<'static> {
    fn from(value: I) -> Self {
        LoadPath::Id(value.into())
    }
}

impl<'a> From<AssetPath<'a>> for LoadPath<'a> {
    fn from(value: AssetPath<'a>) -> Self {
        LoadPath::Path(value)
    }
}

impl<'a> From<&'a str> for LoadPath<'a> {
    fn from(value: &'a str) -> Self {
        LoadPath::Path(AssetPath::from_str(value))
    }
}

impl From<String> for LoadPath<'static> {
    fn from(value: String) -> Self {
        LoadPath::Path(AssetPath::from_string(value))
    }
}

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct Folder {
    pub children: Vec<PathBuf>,
}

impl Asset for Folder {}
impl Settings for Folder {}
