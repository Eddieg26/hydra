use super::{
    AssetIoError, AsyncReader, AsyncWriter, ErasedFileSystem, FileSystem, PathExt, PathStream,
};
use crate::{
    asset::{AssetMetadata, Settings},
    io::{deserialize, serialize},
};
use serde::{Deserialize, Serialize};
use smol::io::AsyncReadExt;
use std::{
    borrow::Cow,
    collections::HashMap,
    hash::Hash,
    path::{Path, PathBuf},
    sync::Arc,
};

#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct AssetPath<'a> {
    source: SourceName<'a>,
    path: Box<Path>,
    name: Option<Box<str>>,
}

impl<'a> AssetPath<'a> {
    pub fn new(source: SourceName<'a>, path: impl AsRef<Path>) -> Self {
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

    pub fn source(&self) -> &SourceName<'a> {
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

    /// remote://assets/texture.png@main
    pub fn from_str(path: &'a str) -> Self {
        let (source, src_index) = match path.find("://") {
            Some(position) => {
                let source = &path[..position];
                (SourceName::Name(source.into()), position + 3)
            }
            None => (SourceName::Default, 0),
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

    pub fn from_string(path: String) -> AssetPath<'static> {
        let (source, src_index) = match path.find("://") {
            Some(position) => {
                let source = &path[..position];
                (SourceName::Name(source.to_string().into()), position + 3)
            }
            None => (SourceName::Default, 0),
        };

        let (name, name_index) = match path[src_index..].find('@') {
            Some(position) => {
                let name = &path[src_index + position + 1..];
                (Some(name.to_string()), src_index + position)
            }
            None => (None, path.len()),
        };

        let path = PathBuf::from(&path[src_index..name_index]);

        AssetPath {
            source,
            path: path.into_boxed_path(),
            name: name.map(|name| name.into_boxed_str()),
        }
    }

    pub fn into_owned(self) -> AssetPath<'static> {
        AssetPath {
            source: self.source.into_owned(),
            path: self.path,
            name: self.name,
        }
    }
}

impl<'a> From<AssetPath<'a>> for PathBuf {
    fn from(value: AssetPath<'a>) -> Self {
        let path = match value.source {
            SourceName::Default => value.path.display().to_string(),
            SourceName::Name(ref name) => format!("{}://{}", name, value.path.display()),
        };

        match &value.name {
            Some(name) => PathBuf::from(format!("{}@{}", path, &name)),
            None => PathBuf::from(path),
        }
    }
}

impl std::fmt::Display for AssetPath<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let path: PathBuf = self.into();
        write!(f, "{}", path.display())
    }
}

impl From<String> for AssetPath<'static> {
    fn from(path: String) -> Self {
        Self::from_string(path)
    }
}

impl<'a> From<&'a str> for AssetPath<'a> {
    fn from(path: &'a str) -> Self {
        Self::from_str(path)
    }
}

impl<'a> From<&'a Path> for AssetPath<'a> {
    fn from(path: &'a Path) -> Self {
        match path.as_os_str().to_str() {
            Some(path) => Self::from_str(path),
            None => Self::new(SourceName::Default, path),
        }
    }
}

impl From<PathBuf> for AssetPath<'static> {
    fn from(path: PathBuf) -> Self {
        if let Some(path) = path.as_os_str().to_str() {
            Self::from_string(path.to_string())
        } else {
            Self::new(SourceName::Default, path)
        }
    }
}

impl Into<PathBuf> for &AssetPath<'_> {
    fn into(self) -> PathBuf {
        let source = match self.source {
            SourceName::Default => "",
            SourceName::Name(ref name) => name,
        };

        let path = format!("{}://{}", source, self.path.display());
        match &self.name {
            Some(name) => PathBuf::from(format!("{}@{}", path, &name)),
            None => PathBuf::from(path),
        }
    }
}

impl<'a> From<&AssetPath<'a>> for AssetPath<'a> {
    fn from(value: &AssetPath<'a>) -> Self {
        value.clone()
    }
}

#[derive(Clone, Default, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize, Hash)]
pub enum SourceName<'a> {
    #[default]
    Default,
    Name(Cow<'a, str>),
}

impl<'a> SourceName<'a> {
    pub fn as_bytes(&self) -> &[u8] {
        match self {
            Self::Default => b"default",
            Self::Name(name) => name.as_bytes(),
        }
    }

    pub fn into_owned(&self) -> SourceName<'static> {
        match self {
            Self::Default => SourceName::Default,
            Self::Name(cow) => SourceName::Name(Cow::Owned(cow.to_string())),
        }
    }
}

impl From<String> for SourceName<'static> {
    fn from(name: String) -> Self {
        SourceName::Name(name.into())
    }
}

impl<'a> From<&'a str> for SourceName<'a> {
    fn from(name: &'a str) -> Self {
        SourceName::Name(name.into())
    }
}

impl std::fmt::Display for SourceName<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SourceName::Default => write!(f, "default"),
            SourceName::Name(name) => write!(f, "{}", name),
        }
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

    pub async fn reader<'a>(
        &'a self,
        path: &'a Path,
    ) -> Result<Box<dyn AsyncReader>, AssetIoError> {
        self.io.reader(path).await
    }

    pub async fn read_dir<'a>(
        &'a self,
        path: &'a Path,
    ) -> Result<Box<dyn PathStream>, AssetIoError> {
        self.io.read_dir(path).await
    }

    pub async fn is_dir<'a>(&'a self, path: &'a Path) -> Result<bool, AssetIoError> {
        self.io.is_dir(path).await
    }

    pub async fn writer<'a>(
        &'a self,
        path: &'a Path,
    ) -> Result<Box<dyn AsyncWriter>, AssetIoError> {
        self.io.writer(path).await
    }

    pub async fn rename<'a>(&'a self, from: &'a Path, to: &'a Path) -> Result<(), AssetIoError> {
        self.io.rename(from, to).await
    }

    pub async fn create_dir<'a>(&'a self, path: &'a Path) -> Result<(), AssetIoError> {
        self.io.create_dir(path).await
    }

    pub async fn create_dir_all<'a>(&'a self, path: &'a Path) -> Result<(), AssetIoError> {
        self.io.create_dir_all(path).await
    }

    pub async fn remove<'a>(&'a self, path: &'a Path) -> Result<(), AssetIoError> {
        self.io.remove(path).await
    }

    pub async fn remove_dir<'a>(&'a self, path: &'a Path) -> Result<(), AssetIoError> {
        self.io.remove_dir(path).await
    }

    pub async fn exists<'a>(&'a self, path: &'a Path) -> Result<bool, AssetIoError> {
        self.io.exists(path).await
    }

    pub fn metadata_path(path: &Path) -> PathBuf {
        path.append_ext("meta")
    }

    pub async fn read_to_string(&self, path: &Path) -> Result<String, AssetIoError> {
        let mut reader = self.reader(path).await?;
        let mut content = String::new();
        reader.read_to_string(&mut content).await?;
        Ok(content)
    }

    pub async fn read_asset_bytes(&self, path: &Path) -> Result<Vec<u8>, AssetIoError> {
        let mut reader = self.reader(path).await?;
        let mut bytes = vec![];
        AsyncReader::read_to_end(&mut reader, &mut bytes).await?;
        Ok(bytes)
    }

    pub async fn read_metadata_bytes(&self, path: &Path) -> Result<Vec<u8>, AssetIoError> {
        let mut reader = self.reader(&Self::metadata_path(path)).await?;
        let mut bytes = vec![];
        AsyncReader::read_to_end(&mut reader, &mut bytes).await?;
        Ok(bytes)
    }

    pub async fn load_metadata<S: Settings + for<'a> Deserialize<'a>>(
        &self,
        path: &Path,
    ) -> Result<AssetMetadata<S>, AssetIoError> {
        let mut reader = self.reader(&Self::metadata_path(path)).await?;
        let mut buffer = Vec::new();

        AsyncReader::read_to_end(&mut reader, &mut buffer).await?;

        deserialize::<AssetMetadata<S>>(&buffer)
    }

    pub async fn save_metadata<S: Settings + Serialize>(
        &self,
        path: &Path,
        metadata: &AssetMetadata<S>,
    ) -> Result<Vec<u8>, AssetIoError> {
        let meta_path = Self::metadata_path(path);
        let mut writer = self.writer(&meta_path).await?;
        let content = serialize(metadata)?;

        use futures::AsyncWriteExt;
        writer.write(&content).await?;
        writer.flush().await?;

        Ok(content)
    }
}

pub struct AssetSources {
    sources: HashMap<SourceName<'static>, AssetSource>,
}

impl AssetSources {
    pub fn new() -> Self {
        Self {
            sources: HashMap::new(),
        }
    }

    pub fn add<F: FileSystem>(&mut self, name: SourceName<'static>, fs: F) {
        self.sources.insert(name, AssetSource::new(fs));
    }

    pub fn get(&self, name: &SourceName<'static>) -> Option<&AssetSource> {
        self.sources.get(name)
    }

    pub fn contains(&self, name: &SourceName) -> bool {
        self.sources.contains_key(name)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&SourceName, &AssetSource)> {
        self.sources.iter()
    }
}

impl std::fmt::Debug for AssetSources {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_map().key(&self.sources.keys().map(|k| k)).finish()
    }
}
