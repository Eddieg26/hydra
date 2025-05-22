use super::{
    AssetIoError, AssetPath, AsyncReader, ErasedFileSystem, FileSystem, deserialize, serialize,
};
use crate::asset::{Asset, AssetId, AssetType, ErasedId};
use serde::{Deserialize, Serialize};
use smol::io::{AsyncReadExt, AsyncWriteExt};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
};

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct AssetLibrary {
    id_map: HashMap<AssetPath<'static>, ErasedId>,
    path_map: HashMap<ErasedId, AssetPath<'static>>,
}

impl AssetLibrary {
    pub fn new() -> Self {
        Self {
            id_map: HashMap::new(),
            path_map: HashMap::new(),
        }
    }

    pub fn get_id<'a>(&'a self, path: &'a AssetPath<'a>) -> Option<&'a ErasedId> {
        self.id_map.get(path)
    }

    pub fn get_path(&self, id: ErasedId) -> Option<&AssetPath<'static>> {
        self.path_map.get(&id)
    }

    pub fn add_asset(&mut self, path: AssetPath<'static>, id: ErasedId) {
        self.id_map.insert(path.clone(), id);
        self.path_map.insert(id, path);
    }

    pub fn remove_asset(&mut self, path: &AssetPath<'static>) -> Option<ErasedId> {
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportMeta {
    pub processor: Option<u32>,
    pub checksum: u32,
    pub full_checksum: u32,
    pub dependencies: Vec<ErasedId>,
}

impl ImportMeta {
    pub fn new(processor: Option<u32>, checksum: u32) -> Self {
        Self {
            processor,
            checksum,
            full_checksum: checksum,
            dependencies: vec![],
        }
    }
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
    pub path: AssetPath<'static>,
    pub import: ImportMeta,
    pub dependencies: Vec<ErasedId>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Artifact {
    pub(crate) header: ArtifactHeader,
    pub(crate) meta: ArtifactMeta,
    pub(crate) data: Vec<u8>,
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

    pub fn id(&self) -> ErasedId {
        self.meta.id
    }

    pub fn ty(&self) -> AssetType {
        self.meta.ty
    }

    pub fn path(&self) -> &AssetPath<'static> {
        &self.meta.path
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

pub struct ArtifactReader(Box<dyn AsyncReader>);

impl ArtifactReader {
    pub fn new(reader: Box<dyn AsyncReader>) -> Self {
        Self(reader)
    }

    pub async fn header(&mut self) -> Result<ArtifactHeader, AssetIoError> {
        let mut buf = [0; std::mem::size_of::<ArtifactHeader>()];
        self.0.read_exact(&mut buf).await?;

        deserialize(&buf)
    }

    pub async fn meta(&mut self, header: &ArtifactHeader) -> Result<ArtifactMeta, AssetIoError> {
        let mut buf = vec![0; header.meta as usize];
        self.0.read_exact(&mut buf).await?;

        deserialize(&buf)
    }

    pub async fn data(mut self) -> Result<Vec<u8>, AssetIoError> {
        let mut buf = vec![];
        AsyncReader::read_to_end(&mut self.0, &mut buf)
            .await
            .map(|_| buf)
    }

    pub async fn into_meta(mut self) -> Result<ArtifactMeta, AssetIoError> {
        let header = self.header().await?;

        self.meta(&header).await
    }

    pub async fn into_artifact(mut self) -> Result<Artifact, AssetIoError> {
        let mut buf = vec![];
        AsyncReader::read_to_end(&mut self.0, &mut buf).await?;

        deserialize(&buf)
    }
}

#[derive(Clone)]
pub struct AssetCache {
    fs: Arc<dyn ErasedFileSystem>,
    artifacts: PathBuf,
    temp: PathBuf,
    library: PathBuf,
}

impl AssetCache {
    pub fn new<F: FileSystem>(fs: F) -> Self {
        let fs = Arc::new(fs);
        let artifacts = fs.root().join("artifacts");
        let temp = fs.root().join(".temp");
        let library = fs.root().join("assets.lib");

        Self {
            fs,
            artifacts,
            temp,
            library,
        }
    }

    pub fn get_artifacts_path(&self) -> &Path {
        &self.artifacts
    }

    pub fn get_temp_path(&self) -> &Path {
        &self.temp
    }

    pub fn get_library_path(&self) -> &Path {
        &self.library
    }

    pub fn get_artifact_path(&self, id: ErasedId) -> PathBuf {
        self.artifacts.join(id.to_string())
    }

    pub fn get_temp_artifact_path(&self, id: ErasedId) -> PathBuf {
        self.temp.join(id.to_string())
    }

    pub async fn get_artifact_reader(&self, id: ErasedId) -> Result<ArtifactReader, AssetIoError> {
        let path = self.get_artifact_path(id);
        self.fs.reader(&path).await.map(ArtifactReader::new)
    }

    pub async fn get_temp_artifact(&self, id: ErasedId) -> Result<Artifact, AssetIoError> {
        let path = self.get_temp_artifact_path(id);
        let mut reader = self.fs.reader(&path).await?;
        let mut data = vec![];
        AsyncReader::read_to_end(&mut reader, &mut data).await?;

        deserialize::<Artifact>(&data)
    }

    pub async fn save_artifact(&self, artifact: &Artifact) -> Result<usize, AssetIoError> {
        let data = serialize(artifact)?;
        let path = self.get_artifact_path(artifact.id());
        let mut writer = self.fs.writer(&path).await?;
        writer.write(&data).await.map_err(AssetIoError::from)
    }

    pub async fn save_temp_artifact(&self, artifact: &Artifact) -> Result<usize, AssetIoError> {
        let data = serialize(artifact)?;
        let path = self.get_temp_artifact_path(artifact.id());
        let mut writer = self.fs.writer(&path).await?;
        writer.write(&data).await.map_err(AssetIoError::from)
    }

    pub async fn remove_artifact(&self, id: ErasedId) -> Result<(), AssetIoError> {
        let path = self.get_artifact_path(id);
        self.fs.remove(&path).await
    }

    pub async fn remove_temp_artifact(&self, id: ErasedId) -> Result<(), AssetIoError> {
        let path = self.get_temp_artifact_path(id);
        self.fs.remove(&path).await
    }

    pub async fn load_asset<A: Asset + for<'a> Deserialize<'a>>(
        &self,
        id: AssetId<A>,
    ) -> Result<LoadedAsset<A>, AssetIoError> {
        let artifact = self
            .get_artifact_reader(id.into())
            .await?
            .into_artifact()
            .await?;
        deserialize::<A>(artifact.data()).map(|asset| LoadedAsset {
            asset,
            meta: artifact.meta,
        })
    }

    pub async fn load_library(&self) -> Result<AssetLibrary, AssetIoError> {
        let mut reader = self.fs.reader(&self.library).await?;
        let mut buf = vec![];
        AsyncReader::read_to_end(&mut reader, &mut buf).await?;

        deserialize(&buf)
    }

    pub async fn save_library(&self, library: &AssetLibrary) -> Result<usize, AssetIoError> {
        let data = serialize(library)?;
        let mut writer = self.fs.writer(&self.library).await?;
        writer.write(&data).await.map_err(AssetIoError::from)
    }

    pub async fn create_temp(&self) -> Result<(), AssetIoError> {
        if !self.fs.exists(&self.temp).await? {
            self.fs.create_dir(&self.temp).await
        } else {
            Ok(())
        }
    }

    pub async fn delete_temp(&self) -> Result<(), AssetIoError> {
        if self.fs.exists(&self.temp).await? {
            self.fs.remove_dir(&self.temp).await
        } else {
            Ok(())
        }
    }
}

pub struct LoadedAsset<A: Asset> {
    pub asset: A,
    pub meta: ArtifactMeta,
}
