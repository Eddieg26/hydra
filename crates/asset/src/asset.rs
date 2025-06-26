use ecs::{Event, Resource, SparseIndex, core::BlobCell};
use serde::{Deserialize, Serialize, de::Visitor, ser::SerializeStruct};
use std::{
    collections::{HashMap, HashSet},
    hash::Hash,
    path::PathBuf,
    str::FromStr,
};

pub trait AssetDependencyReader {
    fn read(&mut self, id: impl Into<ErasedId>);
}

impl AssetDependencyReader for Vec<ErasedId> {
    fn read(&mut self, id: impl Into<ErasedId>) {
        let id = id.into();
        self.push(id);
    }
}

impl AssetDependencyReader for HashSet<ErasedId> {
    fn read(&mut self, id: impl Into<ErasedId>) {
        self.insert(id.into());
    }
}

pub trait AssetDependency: Send + Sync {
    fn get<R: AssetDependencyReader>(&self, reader: &mut R);
}

impl<I: IntoErasedId + Send + Sync + Copy> AssetDependency for Vec<I> {
    fn get<R: AssetDependencyReader>(&self, reader: &mut R) {
        for id in self
            .iter()
            .copied()
            .filter_map(IntoErasedId::into_erased_id)
        {
            reader.read(id);
        }
    }
}

impl<I: IntoErasedId + Send + Sync + Copy> AssetDependency for I {
    fn get<W: AssetDependencyReader>(&self, writer: &mut W) {
        if let Some(id) = self.into_erased_id() {
            writer.read(id);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AssetAction {
    Reload,
    Unload,
}

pub trait Asset: AssetDependency + Send + Sync + 'static {
    const DEPENDENCY_UNLOAD_ACTION: Option<AssetAction> = None;
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

    pub const fn from_u128(value: u128) -> Self {
        Self(uuid::Uuid::from_u128(value), std::marker::PhantomData::<A>)
    }

    pub const fn from_uuid(value: uuid::Uuid) -> Self {
        Self(value, std::marker::PhantomData::<A>)
    }

    pub fn uuid(&self) -> &uuid::Uuid {
        &self.0
    }
}

impl<A: Asset> ToString for AssetId<A> {
    fn to_string(&self) -> String {
        self.0.to_string()
    }
}

impl<A: Asset, O: Asset> AsRef<AssetId<A>> for AssetId<O> {
    fn as_ref(&self) -> &AssetId<A> {
        unsafe { std::mem::transmute(self) }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ErasedId(uuid::Uuid);
impl ErasedId {
    pub const NONE: ErasedId = ErasedId(uuid::Uuid::from_u128(0));

    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4())
    }

    pub fn uuid(&self) -> &uuid::Uuid {
        &self.0
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
        self.0.to_string().serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for ErasedId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        let id = uuid::Uuid::from_str(&value).map_err(serde::de::Error::custom)?;
        Ok(Self(id))
    }
}

impl<A: Asset> Serialize for AssetId<A> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        ErasedId::serialize(&self.0.into(), serializer)
    }
}

impl<'de, A: Asset> Deserialize<'de> for AssetId<A> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        let id = uuid::Uuid::from_str(&value).map_err(serde::de::Error::custom)?;
        Ok(Self(id, Default::default()))
    }
}

impl<A: Asset> std::fmt::Debug for AssetId<A> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!(
            "AssetId::<{}>({})",
            ecs::ext::short_type_name::<A>(),
            self.0
        ))
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

impl<A: Asset> From<&ErasedId> for AssetId<A> {
    fn from(value: &ErasedId) -> Self {
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

impl<A: Asset> From<&AssetId<A>> for ErasedId {
    fn from(value: &AssetId<A>) -> Self {
        ErasedId(value.0)
    }
}

impl From<uuid::Uuid> for ErasedId {
    fn from(value: uuid::Uuid) -> Self {
        Self(value)
    }
}

trait IntoErasedId {
    fn into_erased_id(self) -> Option<ErasedId>;
}

impl IntoErasedId for ErasedId {
    fn into_erased_id(self) -> Option<ErasedId> {
        Some(self)
    }
}

impl IntoErasedId for Option<ErasedId> {
    fn into_erased_id(self) -> Option<ErasedId> {
        self
    }
}

impl<A: Asset> IntoErasedId for AssetId<A> {
    fn into_erased_id(self) -> Option<ErasedId> {
        Some(self.0.into())
    }
}

impl<A: Asset> IntoErasedId for Option<AssetId<A>> {
    fn into_erased_id(self) -> Option<ErasedId> {
        self.map(ErasedId::from)
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

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::SeqAccess<'v>,
            {
                use serde::de::Error;

                let id = seq
                    .next_element::<ErasedId>()?
                    .ok_or(Error::custom("Expected meta id"))?;

                let settings = seq
                    .next_element::<S>()?
                    .ok_or(Error::custom("Expected meta settings"))?;

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

#[derive(Resource)]
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

pub struct Test;

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

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct Folder {
    pub children: Vec<PathBuf>,
}

impl Asset for Folder {}
impl Settings for Folder {}
impl AssetDependency for Folder {
    fn get<W: AssetDependencyReader>(&self, _: &mut W) {}
}

pub struct ErasedAsset(BlobCell);
impl ErasedAsset {
    pub fn new<A: Asset>(asset: A) -> Self {
        Self(BlobCell::new(asset))
    }

    pub fn downcast<A: Asset>(&self) -> &A {
        self.0.get::<A>()
    }

    pub fn downcast_mut<A: Asset>(&mut self) -> &mut A {
        self.0.get_mut::<A>()
    }

    pub fn into<A: Asset>(self) -> A {
        self.0.into_value()
    }
}

impl<A: Asset> From<A> for ErasedAsset {
    fn from(value: A) -> Self {
        Self::new(value)
    }
}
