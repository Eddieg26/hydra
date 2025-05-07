use super::{
    AssetFuture, AssetIoError, AssetReader, AssetWriter, ErasedFileSystem, FileSystem, PathExt,
    PathStream,
};
use crate::asset::{AssetMetadata, Settings};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    hash::Hash,
    path::{Path, PathBuf},
    sync::Arc,
};

#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct AssetPath {
    source: AssetSourceName,
    path: Box<Path>,
    name: Option<Box<str>>,
}

impl From<&AssetPath> for PathBuf {
    fn from(value: &AssetPath) -> Self {
        let source = match value.source {
            AssetSourceName::Default => "",
            AssetSourceName::Name(ref name) => name,
        };

        let path = format!("{}://{}", source, value.path.display());
        match &value.name {
            Some(name) => PathBuf::from(format!("{}@{}", path, &name)),
            None => PathBuf::from(path),
        }
    }
}

impl std::fmt::Display for AssetPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", PathBuf::from(self).display())
    }
}

impl AssetPath {
    pub fn new(source: AssetSourceName, path: impl AsRef<Path>) -> Self {
        Self {
            source,
            path: path.as_ref().to_path_buf().into_boxed_path(),
            name: None,
        }
    }

    pub fn with_name(&self, name: impl Into<Box<str>>) -> Self {
        Self {
            name: Some(name.into()),
            ..self.clone()
        }
    }

    pub fn source(&self) -> &AssetSourceName {
        &self.source
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn ext(&self) -> Option<&str> {
        self.path.ext()
    }

    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    pub fn artifact_name(&self) -> PathBuf {
        let mut hasher = blake3::Hasher::new();
        hasher.update(self.path.as_ref().as_os_str().as_encoded_bytes());
        hasher.update(self.source.as_bytes());
        if let Some(name) = &self.name {
            hasher.update(name.as_bytes());
        }

        let hash = hasher.finalize();
        let name = String::from_utf8_lossy(hash.as_bytes());

        PathBuf::from(name.to_string())
    }

    /// remote://assets/texture.png@main
    pub fn from_str<A: AsRef<str>>(path: A) -> Self {
        let path = path.as_ref();
        let (source, src_index) = match path.find("://") {
            Some(position) => {
                let source = &path[..position];
                (AssetSourceName::Name(source.to_string()), position + 3)
            }
            None => (AssetSourceName::Default, 0),
        };

        let (name, name_index) = match path[src_index..].find('@') {
            Some(position) => {
                let name = &path[src_index + position + 1..];
                (Some(name.to_string()), src_index + position)
            }
            None => (None, path.len()),
        };

        let path = &path[src_index..name_index];
        let path = Path::new(path);

        Self {
            source,
            path: path.to_path_buf().into_boxed_path(),
            name: name.map(|name| name.into_boxed_str()),
        }
    }
}

impl From<String> for AssetPath {
    fn from(path: String) -> Self {
        Self::from_str(path)
    }
}

impl From<&str> for AssetPath {
    fn from(path: &str) -> Self {
        Self::from_str(path)
    }
}

impl From<&Path> for AssetPath {
    fn from(path: &Path) -> Self {
        match path.as_os_str().to_str() {
            Some(path) => Self::from_str(path),
            None => Self::new(AssetSourceName::Default, path),
        }
    }
}

impl From<PathBuf> for AssetPath {
    fn from(path: PathBuf) -> Self {
        Self::from(path.as_path())
    }
}

impl From<&AssetPath> for AssetPath {
    fn from(value: &AssetPath) -> Self {
        value.clone()
    }
}

#[derive(Clone, Default, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize, Hash)]
pub enum AssetSourceName {
    #[default]
    Default,
    Name(String),
}

impl AssetSourceName {
    pub fn as_bytes(&self) -> &[u8] {
        match self {
            AssetSourceName::Default => b"default",
            AssetSourceName::Name(name) => name.as_bytes(),
        }
    }
}

impl From<String> for AssetSourceName {
    fn from(name: String) -> Self {
        AssetSourceName::Name(name)
    }
}

impl From<&str> for AssetSourceName {
    fn from(name: &str) -> Self {
        AssetSourceName::Name(name.to_string())
    }
}

#[derive(Clone)]
pub struct AssetSource {
    io: Arc<dyn ErasedFileSystem>,
}

impl AssetSource {
    pub fn new<I: FileSystem>(io: I) -> Self {
        Self { io: Arc::new(io) }
    }

    pub fn io(&self) -> &dyn ErasedFileSystem {
        self.io.as_ref()
    }

    pub fn reader<'a>(&'a self, path: &'a Path) -> AssetFuture<'a, Box<dyn AssetReader>> {
        self.io.reader(path)
    }

    pub async fn read_dir(
        &self,
        path: impl AsRef<Path>,
    ) -> Result<Box<dyn PathStream>, AssetIoError> {
        self.io.read_dir(path.as_ref()).await
    }

    pub fn is_dir<'a>(&'a self, path: &'a Path) -> AssetFuture<'a, bool> {
        self.io.is_dir(path)
    }

    pub fn writer<'a>(&'a self, path: &'a Path) -> AssetFuture<'a, Box<dyn AssetWriter>> {
        self.io.writer(path)
    }

    pub fn rename<'a>(&'a self, from: &'a Path, to: &'a Path) -> AssetFuture<'a, ()> {
        self.io.rename(from, to)
    }

    pub fn create_dir<'a>(&'a self, path: &'a Path) -> AssetFuture<'a, ()> {
        self.io.create_dir(path)
    }

    pub fn create_dir_all<'a>(&'a self, path: &'a Path) -> AssetFuture<'a, ()> {
        self.io.create_dir_all(path)
    }

    pub fn remove<'a>(&'a self, path: &'a Path) -> AssetFuture<'a, ()> {
        self.io.remove(path)
    }

    pub fn remove_dir<'a>(&'a self, path: &'a Path) -> AssetFuture<'a, ()> {
        self.io.remove_dir(path)
    }

    pub fn exists<'a>(&'a self, path: &'a Path) -> AssetFuture<'a, bool> {
        self.io.exists(path)
    }

    pub fn metadata_path(path: &Path) -> PathBuf {
        path.append_ext("meta")
    }

    pub async fn read_asset_bytes(&self, path: &Path) -> Result<Vec<u8>, AssetIoError> {
        let mut reader = self.reader(path).await?;
        let mut bytes = vec![];
        reader.read_to_end(&mut bytes).await?;
        Ok(bytes)
    }

    pub async fn read_metadata_bytes(&self, path: &Path) -> Result<Vec<u8>, AssetIoError> {
        let mut reader = self.reader(&Self::metadata_path(path)).await?;
        let mut bytes = vec![];
        reader.read_to_end(&mut bytes).await?;
        Ok(bytes)
    }

    pub async fn load_metadata<S: Settings + for<'a> Deserialize<'a>>(
        &self,
        path: &Path,
    ) -> Result<AssetMetadata<S>, AssetIoError> {
        let mut reader = self.reader(&Self::metadata_path(path)).await?;
        let mut buffer = String::new();

        use futures::AsyncReadExt;
        reader.read_to_string(&mut buffer).await?;

        ron::from_str::<AssetMetadata<S>>(&buffer).map_err(AssetIoError::from)
    }

    pub async fn save_metadata<S: Settings + Serialize>(
        &self,
        path: &Path,
        settings: &AssetMetadata<S>,
    ) -> Result<String, AssetIoError> {
        let meta_path = Self::metadata_path(path);
        let mut writer = self.writer(&meta_path).await?;
        let content = ron::to_string(settings).map_err(AssetIoError::from)?;

        use futures::AsyncWriteExt;
        writer.write(content.as_bytes()).await?;
        writer.flush().await?;

        Ok(content)
    }
}

pub struct AssetSources {
    sources: HashMap<AssetSourceName, AssetSource>,
}

impl AssetSources {
    pub fn new() -> Self {
        Self {
            sources: HashMap::new(),
        }
    }

    pub fn add<I: FileSystem>(&mut self, name: AssetSourceName, io: I) {
        self.sources.insert(name, AssetSource::new(io));
    }

    pub fn get(&self, name: &AssetSourceName) -> Option<&AssetSource> {
        self.sources.get(name)
    }

    pub fn contains(&self, name: &AssetSourceName) -> bool {
        self.sources.contains_key(name)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&AssetSourceName, &AssetSource)> {
        self.sources.iter()
    }
}
