use super::{AssetIoError, AsyncReader, ErasedFileSystem, FileSystem, deserialize, serialize};
use crate::asset::{Asset, AssetType, ErasedId};
use serde::{Deserialize, Serialize};
use smol::io::{AsyncReadExt, AsyncWriteExt};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
};

#[derive(Debug, Serialize, Deserialize)]
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

    pub fn id(&self) -> ErasedId {
        self.meta.id
    }

    pub fn ty(&self) -> AssetType {
        self.meta.ty
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

    pub async fn get_artifact_reader(&self, id: ErasedId) -> Result<ArtifactReader, AssetIoError> {
        let path = self.get_artifact_path(id);
        self.fs.reader(&path).await.map(ArtifactReader::new)
    }

    pub async fn save_artifact(&self, artifact: &Artifact) -> Result<usize, AssetIoError> {
        let data = serialize(artifact)?;
        let path = self.get_artifact_path(artifact.id());
        let mut writer = self.fs.writer(&path).await?;
        writer.write(&data).await.map_err(AssetIoError::from)
    }

    pub async fn remove_artifact(&self, id: ErasedId) -> Result<(), AssetIoError> {
        let path = self.get_artifact_path(id);
        self.fs.remove(&path).await
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
}
