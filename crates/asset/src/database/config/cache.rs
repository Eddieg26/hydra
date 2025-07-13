use crate::{
    asset::{Asset, AssetId, AssetType, ErasedId},
    database::{config::importer::AssetProcessorId, library::AssetLibrary},
    ext::{DeserializeExt, SerializeExt},
    io::{AsyncIoError, AsyncReader, ErasedFileSystem, FileSystem, path::AssetPath},
};
use futures::{AsyncReadExt, AsyncWriteExt};
use serde::{Deserialize, Serialize, ser::SerializeStruct};
use std::{hash::Hash, path::PathBuf};

#[derive(
    Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Hash,
)]
pub struct Checksum(u32);

impl Checksum {
    pub fn new(bytes: &[u8]) -> Self {
        Self(crc32fast::hash(bytes))
    }

    pub fn from(asset: &[u8], settings: &[u8]) -> Self {
        let mut hasher = crc32fast::Hasher::new();
        asset.hash(&mut hasher);
        settings.hash(&mut hasher);

        Self(hasher.finalize())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ImportDependency {
    pub id: ErasedId,
    pub checksum: Checksum,
}

impl ImportDependency {
    pub fn new(id: ErasedId, checksum: Checksum) -> Self {
        Self { id, checksum }
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct ImportInfo {
    pub processor: Option<AssetProcessorId>,
    pub checksum: Checksum,
    pub full_checksum: Checksum,
    pub dependencies: Vec<ImportDependency>,
}

impl ImportInfo {
    pub fn set_dependencies(&mut self, dependencies: Vec<ImportDependency>) {
        let mut hasher = crc32fast::Hasher::new();
        self.checksum.hash(&mut hasher);

        for dep in &dependencies {
            dep.checksum.hash(&mut hasher);
        }

        self.full_checksum = Checksum(hasher.finalize());
        self.dependencies = dependencies;
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetMetadata {
    pub id: ErasedId,
    pub ty: AssetType,
    pub path: AssetPath<'static>,
    pub import: ImportInfo,
    pub dependencies: Vec<ErasedId>,
    pub parent: Option<ErasedId>,
    pub children: Vec<ErasedId>,
}

impl AssetMetadata {
    pub fn new(id: ErasedId, ty: AssetType, path: AssetPath<'static>) -> Self {
        Self {
            id,
            ty,
            path,
            import: ImportInfo::default(),
            dependencies: Vec::new(),
            parent: None,
            children: Vec::new(),
        }
    }

    pub fn with_parent(mut self, parent: ErasedId) -> Self {
        self.parent = Some(parent);
        self
    }

    pub fn with_children(mut self, children: Vec<ErasedId>) -> Self {
        self.children = children;
        self
    }

    pub fn with_dependencies(mut self, dependencies: Vec<ErasedId>) -> Self {
        self.dependencies = dependencies;
        self
    }

    pub fn with_checksum(mut self, checksum: Checksum) -> Self {
        self.import.checksum = checksum;
        self
    }

    pub fn with_processor(mut self, processor: Option<AssetProcessorId>) -> Self {
        self.import.processor = processor;
        self
    }

    pub fn all_dependencies(&self) -> impl Iterator<Item = &ErasedId> {
        self.dependencies.iter().chain(self.parent.iter())
    }
}

pub struct Artifact {
    pub(crate) meta: AssetMetadata,
    pub(crate) data: Vec<u8>,
}

impl Artifact {
    pub unsafe fn from_raw_parts(data: Vec<u8>, meta: AssetMetadata) -> Self {
        Self { meta, data }
    }

    pub fn id(&self) -> ErasedId {
        self.meta.id
    }

    pub fn ty(&self) -> AssetType {
        self.meta.ty
    }

    pub fn path(&self) -> &AssetPath<'static> {
        &self.meta.path
    }

    pub fn meta(&self) -> &AssetMetadata {
        &self.meta
    }

    pub fn data(&self) -> &[u8] {
        &self.data
    }
}

impl Serialize for Artifact {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut artifact = serializer.serialize_struct("Artifact", 3)?;
        let meta = self.meta.to_bytes().map_err(serde::ser::Error::custom)?;
        let size: [u8; 4] = (meta.len() as u32).to_le_bytes();

        artifact.serialize_field("header", &size)?;
        artifact.serialize_field("meta", &self.meta)?;
        artifact.serialize_field("data", &self.data)?;

        artifact.end()
    }
}

impl<'de> Deserialize<'de> for Artifact {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct Visitor;

        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = Artifact;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("struct Artifact")
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::SeqAccess<'de>,
            {
                use serde::de::Error;

                let _ = seq
                    .next_element::<[u8; 4]>()?
                    .ok_or(Error::custom("Expected meta size"))?;

                let meta = seq
                    .next_element::<AssetMetadata>()?
                    .ok_or(Error::custom("Expected meta"))?;

                let data = seq
                    .next_element::<Vec<u8>>()?
                    .ok_or(Error::custom("Expected data bytes"))?;

                Ok(Artifact { meta, data })
            }
        }

        deserializer.deserialize_struct("Artifact", &["meta_size", "meta", "data"], Visitor)
    }
}

pub struct LoadedAsset<A: Asset> {
    pub asset: A,
    pub meta: AssetMetadata,
}

impl<A: Asset> LoadedAsset<A> {
    pub fn new(asset: A, meta: AssetMetadata) -> Self {
        Self { asset, meta }
    }
}

pub struct ArtifactReader(Box<dyn AsyncReader>);

impl ArtifactReader {
    pub async fn read(mut self) -> Result<Artifact, AsyncIoError> {
        let mut buf = Vec::new();
        AsyncReader::read_to_end(&mut self.0, &mut buf).await?;
        Artifact::from_bytes(&buf).map_err(AsyncIoError::from)
    }

    pub async fn read_metadata(mut self) -> Result<AssetMetadata, AsyncIoError> {
        let mut buf = [0u8; 4];
        self.0.read_exact(&mut buf).await?;

        let len = u32::from_le_bytes(buf) as usize;
        let mut buf = vec![0u8; len];
        self.0.read_exact(&mut buf).await?;

        AssetMetadata::from_bytes(&buf).map_err(AsyncIoError::from)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArtifactPath {
    /// Path to the artifact in the cache
    Cache,
    /// Path to the artifact in the temp directory
    Source,
}

impl ArtifactPath {
    pub fn as_str(&self) -> &'static str {
        match self {
            ArtifactPath::Cache => "artifacts",
            ArtifactPath::Source => "sources",
        }
    }
}

impl std::fmt::Display for ArtifactPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

pub struct AssetCache {
    fs: Box<dyn ErasedFileSystem>,
}

impl AssetCache {
    pub const LIBRARY: &'static str = "assets.lib";

    pub fn new<F: FileSystem>(fs: F) -> Self {
        Self { fs: Box::new(fs) }
    }

    pub fn fs(&self) -> &dyn ErasedFileSystem {
        self.fs.as_ref()
    }

    pub fn artifact_path(&self, path: ArtifactPath, id: &ErasedId) -> PathBuf {
        self.fs.root().join(path.as_str()).join(id.to_string())
    }

    pub async fn exists(&self, path: ArtifactPath, id: &ErasedId) -> Result<bool, AsyncIoError> {
        let path = self.fs.root().join(path.as_str()).join(id.to_string());
        self.fs.exists(&path).await
    }

    pub async fn load<A: Asset + for<'de> Deserialize<'de>>(
        &self,
        id: AssetId<A>,
    ) -> Result<LoadedAsset<A>, AsyncIoError> {
        let artifact = self
            .artifact_reader(ArtifactPath::Cache, &id.into())
            .await?
            .read()
            .await?;
        let asset = A::from_bytes(artifact.data()).map_err(AsyncIoError::from)?;

        Ok(LoadedAsset {
            asset,
            meta: artifact.meta,
        })
    }

    pub async fn artifact_reader(
        &self,
        path: ArtifactPath,
        id: &ErasedId,
    ) -> Result<ArtifactReader, AsyncIoError> {
        let path = self.artifact_path(path, id);
        self.fs.reader(&path).await.map(ArtifactReader)
    }

    pub async fn read_artifact(
        &self,
        path: ArtifactPath,
        id: &ErasedId,
    ) -> Result<Artifact, AsyncIoError> {
        let path = self.artifact_path(path, id);
        let mut reader = self.fs.reader(&path).await?;
        let mut data = Vec::new();

        AsyncReader::read_to_end(&mut reader, &mut data)
            .await
            .map_err(AsyncIoError::from)?;

        Artifact::from_bytes(&data).map_err(AsyncIoError::from)
    }

    pub async fn save_artifact(
        &self,
        path: ArtifactPath,
        artifact: &Artifact,
    ) -> Result<(), AsyncIoError> {
        let path = self.artifact_path(path, &artifact.id());
        let data = artifact.to_bytes().map_err(AsyncIoError::from)?;
        let mut writer = self.fs.writer(&path).await?;

        writer.write_all(&data).await.map_err(AsyncIoError::from)?;
        writer.flush().await.map_err(AsyncIoError::from)
    }

    pub async fn remove_artifact(
        &self,
        path: ArtifactPath,
        id: &ErasedId,
    ) -> Result<(), AsyncIoError> {
        self.fs.remove(&self.artifact_path(path, id)).await
    }

    pub async fn remove_artifacts(&self, path: ArtifactPath) -> Result<(), AsyncIoError> {
        let path = self.fs.root().join(path.as_str());
        self.fs.remove_dir(&path).await
    }

    pub async fn save_library(&self, library: &AssetLibrary) -> Result<Vec<u8>, AsyncIoError> {
        let mut writer = self.fs.writer(Self::LIBRARY.as_ref()).await?;
        let data = library.to_bytes().map_err(AsyncIoError::from)?;
        writer.write_all(&data).await.map_err(AsyncIoError::from)?;
        writer.flush().await.map_err(AsyncIoError::from)?;

        Ok(data)
    }

    pub async fn load_library(&self) -> Result<AssetLibrary, AsyncIoError> {
        let mut reader = self.fs.reader(Self::LIBRARY.as_ref()).await?;
        let mut data = Vec::new();
        AsyncReader::read_to_end(&mut reader, &mut data).await?;
        AssetLibrary::from_bytes(&data).map_err(AsyncIoError::from)
    }

    pub async fn get_full_checksum(
        &self,
        checksum: Checksum,
        meta: &AssetMetadata,
    ) -> Option<Checksum> {
        let mut hasher = crc32fast::Hasher::new();
        checksum.hash(&mut hasher);

        for dep in &meta.import.dependencies {
            let artifact = self
                .artifact_reader(ArtifactPath::Cache, &dep.id)
                .await
                .ok()?;

            let meta = artifact.read_metadata().await.ok()?;
            meta.import.full_checksum.hash(&mut hasher);
        }

        Some(Checksum(hasher.finalize()))
    }
}
