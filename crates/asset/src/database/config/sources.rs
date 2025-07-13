use crate::{
    ext::{DeserializeExt, PathExt, SerializeExt},
    io::{
        AsyncIoError, AsyncReader, AsyncWriter, BoxFuture, ErasedFileSystem, FileSystem,
        PathStream, path::AssetSource,
    },
    settings::{AssetSettings, Settings},
};
use smol::io::AsyncWriteExt;
use std::{collections::HashMap, path::Path};

pub struct AssetFileSystem(Box<dyn ErasedFileSystem>);

impl AssetFileSystem {
    pub fn new<F: FileSystem>(fs: F) -> Self {
        Self(Box::new(fs))
    }

    pub fn path(&self) -> &Path {
        self.0.root()
    }

    pub fn reader<'a>(
        &'a self,
        path: &'a Path,
    ) -> BoxFuture<'a, Result<Box<dyn AsyncReader>, AsyncIoError>> {
        self.0.reader(path)
    }

    pub fn writer<'a>(
        &'a self,
        path: &'a Path,
    ) -> BoxFuture<'a, Result<Box<dyn AsyncWriter>, AsyncIoError>> {
        self.0.writer(path)
    }

    pub fn read_dir<'a>(
        &'a self,
        path: &'a Path,
    ) -> BoxFuture<'a, Result<Box<dyn PathStream>, AsyncIoError>> {
        self.0.read_dir(path)
    }

    pub fn is_dir<'a>(&'a self, path: &'a Path) -> BoxFuture<'a, Result<bool, AsyncIoError>> {
        self.0.is_dir(path)
    }

    pub fn exists<'a>(&'a self, path: &'a Path) -> BoxFuture<'a, Result<bool, AsyncIoError>> {
        self.0.exists(path)
    }

    pub fn create_dir<'a>(&'a self, path: &'a Path) -> BoxFuture<'a, Result<(), AsyncIoError>> {
        self.0.create_dir(path)
    }

    pub async fn read(&self, path: &Path) -> Result<Vec<u8>, AsyncIoError> {
        let mut reader = self.reader(path).await?;
        let mut data = Vec::new();
        reader.read_to_end(&mut data).await?;

        Ok(data)
    }

    pub async fn write(self, path: &Path, data: &[u8]) -> Result<(), AsyncIoError> {
        let mut writer = self.writer(path).await?;
        writer.write_all(data).await.map_err(AsyncIoError::from)?;
        AsyncWriter::flush(&mut writer).await
    }

    pub async fn load_settings<S: Settings + DeserializeExt>(
        &self,
        path: &Path,
    ) -> Result<AssetSettings<S>, AsyncIoError> {
        let mut reader = self.reader(&path.append_ext("meta")).await?;
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;

        AssetSettings::<S>::from_bytes(&bytes).map_err(AsyncIoError::from)
    }

    pub async fn save_settings<S: Settings + SerializeExt>(
        &self,
        path: &Path,
        settings: &AssetSettings<S>,
    ) -> Result<Vec<u8>, AsyncIoError> {
        let bytes = settings.to_bytes().map_err(AsyncIoError::from)?;
        let mut writer = self.writer(&path.append_ext("meta")).await?;
        writer.write_all(&bytes).await.map_err(AsyncIoError::from)?;
        AsyncWriter::flush(&mut writer).await?;

        Ok(bytes)
    }
}

pub struct AssetSources(HashMap<AssetSource<'static>, AssetFileSystem>);
impl AssetSources {
    pub fn new() -> Self {
        Self(HashMap::new())
    }

    pub fn add<F: FileSystem>(&mut self, source: impl Into<AssetSource<'static>>, fs: F) {
        let source = source.into();
        self.0.insert(source, AssetFileSystem::new(fs));
    }

    pub fn get<'a>(&'a self, source: &'a AssetSource) -> Option<&'a AssetFileSystem> {
        self.0.get(source)
    }

    pub fn iter(&self) -> impl Iterator<Item = AssetFileSource<'_>> {
        self.0
            .iter()
            .map(|(source, fs)| AssetFileSource { name: source, fs })
    }

    pub fn contains(&self, source: &AssetSource<'static>) -> bool {
        self.0.contains_key(source)
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

pub struct AssetFileSource<'a> {
    pub name: &'a AssetSource<'static>,
    fs: &'a AssetFileSystem,
}

impl std::ops::Deref for AssetFileSource<'_> {
    type Target = AssetFileSystem;

    fn deref(&self) -> &Self::Target {
        self.fs
    }
}
