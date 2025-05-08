use super::{
    AssetIoError, ErasedFileSystem, FileSystem, deserialize, local::LocalFs, serialize,
    source::AssetPath,
};
use crate::asset::{Asset, AssetId, AssetType, ErasedId};
use futures::{AsyncReadExt, AsyncWriteExt};
use serde::{Deserialize, Serialize};
use smol::lock::RwLock;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
};

pub struct AssetCache {
    fs: Box<dyn ErasedFileSystem>,
}

impl AssetCache {
    pub fn new(path: impl AsRef<Path>) -> Self {
        Self {
            fs: Box::new(LocalFs::new(path)),
        }
    }

    pub fn with_fs<F: FileSystem>(fs: F) -> Self {
        Self { fs: Box::new(fs) }
    }

    pub async fn init(&self) -> Result<AssetLibrary, AssetIoError> {
        let root = self.fs.root();
        if !root.exists() {
            self.fs.create_dir_all(&root).await?;
        }
        let artifacts = root.join("artifacts");
        if !artifacts.exists() {
            self.fs.create_dir_all(&artifacts).await?;
        }

        let library = match self.load_library().await {
            Ok(library) => library,
            Err(_) => {
                let library = AssetLibrary::new();
                self.save_library(&library).await?;
                library
            }
        };

        Ok(library)
    }

    pub fn root(&self) -> &Path {
        self.fs.root()
    }

    pub fn artifact_path(&self, id: &ErasedId) -> PathBuf {
        self.fs.root().join("artifacts").join(id.to_string())
    }

    pub fn unprocessed_artifact_path(&self, id: &ErasedId) -> PathBuf {
        self.fs.root().join("temp").join(id.to_string())
    }

    pub async fn create_unprocessed_dir(&self) -> Result<(), AssetIoError> {
        self.fs.create_dir_all(&self.fs.root().join("temp")).await
    }

    pub async fn remove_unprocessed_dir(&self) -> Result<(), AssetIoError> {
        let path = self.fs.root().join("temp");
        self.fs.remove_dir(&path).await
    }

    pub async fn load_asset<A: Asset + for<'a> Deserialize<'a>>(
        &self,
        id: &ErasedId,
    ) -> Result<LoadedAsset<A>, AssetIoError> {
        let path = self.artifact_path(id);
        let artifact = self.load_artifact(&path).await?;
        let asset = deserialize::<A>(&artifact.data).map_err(AssetIoError::from)?;
        Ok(LoadedAsset::new(asset, artifact.meta))
    }

    pub async fn load_artifact(&self, path: &Path) -> Result<Artifact, AssetIoError> {
        let mut file = self.fs.reader(&path).await?;
        let mut header = [0; std::mem::size_of::<ArtifactHeader>()];
        file.read_exact(&mut header).await?;
        let header = deserialize::<ArtifactHeader>(&header).map_err(AssetIoError::from)?;
        let mut meta = vec![0; header.size as usize];
        file.read_exact(&mut meta).await?;
        let meta = deserialize::<ArtifactMeta>(&meta).map_err(AssetIoError::from)?;
        let mut data = Vec::new();
        file.read_to_end(&mut data).await?;
        Ok(Artifact { meta, data })
    }

    pub async fn save_artifact(
        &self,
        path: &Path,
        artifact: &Artifact,
    ) -> Result<(), AssetIoError> {
        let mut file = self.fs.writer(&path).await?;
        let meta = serialize(&artifact.meta).map_err(AssetIoError::from)?;
        let mut data =
            serialize(&ArtifactHeader::new(meta.len() as u64)).map_err(AssetIoError::from)?;
        data.extend_from_slice(&meta);
        data.extend_from_slice(&artifact.data);
        file.write_all(&data).await.map_err(AssetIoError::from)
    }

    pub async fn load_artifact_meta(&self, id: &ErasedId) -> Result<ArtifactMeta, AssetIoError> {
        let path = self.artifact_path(id);
        let mut file = self.fs.reader(&path).await?;
        let mut header = [0; std::mem::size_of::<ArtifactHeader>()];
        file.read_exact(&mut header).await?;
        let header = deserialize::<ArtifactHeader>(&header).map_err(AssetIoError::from)?;
        let mut meta = vec![0; header.size as usize];
        file.read_exact(&mut meta).await?;
        deserialize(&meta).map_err(AssetIoError::from)
    }

    pub fn artifact_exists(&self, id: &ErasedId) -> bool {
        self.artifact_path(id).exists()
    }

    pub fn remove_artifact<'a>(
        &'a self,
        path: &'a Path,
    ) -> impl futures::Future<Output = Result<(), AssetIoError>> + 'a {
        self.fs.remove(path)
    }

    pub async fn load_library(&self) -> Result<AssetLibrary, AssetIoError> {
        let path = self.fs.root().join("assets.lib");
        if !path.exists() {
            return Ok(AssetLibrary::new());
        }

        let mut file = self.fs.reader(&path).await?;
        let mut data = String::new();
        file.read_to_string(&mut data).await?;
        ron::from_str(&data).map_err(AssetIoError::from)
    }

    pub async fn save_library(&self, map: &AssetLibrary) -> Result<(), AssetIoError> {
        let path = self.fs.root().join("assets.lib");
        let mut file = self.fs.writer(&path).await?;
        let data = ron::to_string(map)
            .map_err(AssetIoError::from)?
            .into_bytes();
        file.write_all(&data).await.map_err(AssetIoError::from)
    }
}

pub struct LoadedAsset<A: Asset> {
    asset: A,
    meta: ArtifactMeta,
}

impl<A: Asset> LoadedAsset<A> {
    pub fn new(asset: A, meta: ArtifactMeta) -> Self {
        Self { asset, meta }
    }

    pub fn asset(&self) -> &A {
        &self.asset
    }

    pub fn meta(&self) -> &ArtifactMeta {
        &self.meta
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct ArtifactHeader {
    /// The size of the artifact metadata in bytes.
    size: u64,
}

impl ArtifactHeader {
    pub fn new(size: u64) -> Self {
        Self { size }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ArtifactMeta {
    pub id: ErasedId,
    pub ty: AssetType,
    pub checksum: u32,
    pub path: AssetPath,
    pub parent: Option<ErasedId>,
    pub children: Vec<ErasedId>,
    pub dependencies: Vec<ErasedId>,
    pub processed: Option<ProcessedInfo>,
}

impl ArtifactMeta {
    pub fn new(id: ErasedId, ty: AssetType, path: AssetPath, checksum: u32) -> Self {
        Self {
            id,
            ty,
            checksum,
            path,
            parent: None,
            children: Vec::new(),
            dependencies: Vec::new(),
            processed: None,
        }
    }

    pub fn with_dependencies(mut self, dependencies: Vec<ErasedId>) -> Self {
        self.dependencies = dependencies;
        self
    }

    pub fn with_parent(mut self, parent: ErasedId) -> Self {
        self.parent = Some(parent);
        self
    }

    pub fn with_children(mut self, children: Vec<ErasedId>) -> Self {
        self.children = children;
        self
    }

    pub fn with_processed(mut self, processed: ProcessedInfo) -> Self {
        self.processed = Some(processed);
        self
    }

    pub fn calculate_checksum(asset: &[u8], metadata: &[u8]) -> u32 {
        let mut hasher = crc32fast::Hasher::new();
        hasher.update(asset);
        hasher.update(metadata);
        hasher.finalize()
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct Artifact {
    pub meta: ArtifactMeta,
    pub data: Vec<u8>,
}

impl Artifact {
    pub fn from_asset<A: Asset + Serialize>(
        asset: &A,
        meta: ArtifactMeta,
    ) -> Result<Self, bincode::error::EncodeError> {
        Ok(Self {
            meta,
            data: serialize(asset)?,
        })
    }

    pub fn id(&self) -> ErasedId {
        self.meta.id
    }

    pub fn ty(&self) -> AssetType {
        self.meta.ty
    }

    pub fn path(&self) -> &AssetPath {
        &self.meta.path
    }

    pub fn deserialize<A: Asset + for<'a> Deserialize<'a>>(
        &self,
    ) -> Result<A, bincode::error::DecodeError> {
        deserialize(&self.data)
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ProcessedInfo {
    pub dependencies: Vec<AssetInfo>,
}

impl ProcessedInfo {
    pub fn new() -> Self {
        Self {
            dependencies: Vec::new(),
        }
    }

    pub fn with_dependencies(mut self, dependencies: Vec<AssetInfo>) -> Self {
        self.dependencies = dependencies;
        self
    }

    pub fn add_dependency(&mut self, id: ErasedId, checksum: u32) {
        self.dependencies.push(AssetInfo { id, checksum });
    }
}

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct AssetInfo {
    pub id: ErasedId,
    pub checksum: u32,
}

impl AssetInfo {
    pub fn new(id: ErasedId, checksum: u32) -> Self {
        Self { id, checksum }
    }
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct AssetLibrary {
    assets: HashMap<AssetPath, AssetInfo>,
    paths: HashMap<ErasedId, AssetPath>,
}

impl AssetLibrary {
    pub fn new() -> Self {
        Self {
            assets: HashMap::new(),
            paths: HashMap::new(),
        }
    }

    pub fn add(&mut self, path: AssetPath, info: AssetInfo) {
        self.paths.insert(info.id, path.clone());
        self.assets.insert(path, info);
    }

    pub fn remove(&mut self, path: &AssetPath) -> Option<AssetInfo> {
        let info = self.assets.remove(path)?;
        self.paths.remove(&info.id);
        Some(info)
    }

    pub fn get(&self, path: &AssetPath) -> Option<AssetInfo> {
        self.assets.get(path).copied()
    }

    pub fn get_id(&self, path: &AssetPath) -> Option<ErasedId> {
        self.assets.get(path).map(|info| info.id)
    }

    pub fn get_path(&self, id: &ErasedId) -> Option<&AssetPath> {
        self.paths.get(id)
    }

    pub fn contains(&self, path: &AssetPath) -> bool {
        self.assets.contains_key(path)
    }

    pub fn len(&self) -> usize {
        self.assets.len()
    }

    pub fn is_empty(&self) -> bool {
        self.assets.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&AssetPath, &AssetInfo)> {
        self.assets.iter()
    }

    pub fn paths(&self) -> impl Iterator<Item = &AssetPath> {
        self.assets.keys()
    }

    pub fn ids(&self) -> impl Iterator<Item = &ErasedId> {
        self.paths.keys()
    }

    pub fn clear(&mut self) {
        self.assets.clear();
    }
}

pub type SharedLibrary = Arc<RwLock<AssetLibrary>>;

#[derive(Clone, PartialEq, Eq, Hash)]
pub enum LoadPath<A: Asset> {
    Id(AssetId<A>),
    Path(AssetPath),
}

impl<A: Asset> From<ErasedId> for LoadPath<A> {
    fn from(id: ErasedId) -> Self {
        LoadPath::Id(id.into())
    }
}

impl<A: Asset> From<AssetId<A>> for LoadPath<A> {
    fn from(id: AssetId<A>) -> Self {
        LoadPath::Id(id)
    }
}

impl<A: Asset> From<uuid::Uuid> for LoadPath<A> {
    fn from(id: uuid::Uuid) -> Self {
        LoadPath::Id(id.into())
    }
}

impl<A: Asset, I: Into<AssetPath>> From<I> for LoadPath<A> {
    fn from(path: I) -> Self {
        LoadPath::Path(path.into())
    }
}

impl<A: Asset> std::fmt::Debug for LoadPath<A> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Id(arg0) => f.debug_tuple("Id").field(arg0).finish(),
            Self::Path(arg0) => f.debug_tuple("Path").field(arg0).finish(),
        }
    }
}

impl<A: Asset> std::fmt::Display for LoadPath<A> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(self, f)
    }
}
