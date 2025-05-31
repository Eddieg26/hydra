use futures::{AsyncRead, AsyncWrite, Stream};
use serde::{Deserialize, Serialize};
use std::{
    borrow::Cow,
    future::Future,
    path::{Path, PathBuf},
    pin::Pin,
    sync::Arc,
};
use thiserror::Error;

pub mod cache;
pub mod embedded;
pub mod local;
pub mod source;
pub mod vfs;

pub use cache::*;
pub use embedded::*;
pub use local::*;
pub use source::*;
pub use vfs::*;

#[derive(Clone, Error, Debug)]
pub enum AssetIoError {
    #[error("Path not found: {0}")]
    NotFound(PathBuf),

    #[error("{0}")]
    Io(Arc<std::io::Error>),

    #[error("Http error {0}")]
    Http(u16),

    #[error("{0}")]
    Unknown(String),
}

impl AssetIoError {
    pub fn unknown(value: impl ToString) -> Self {
        Self::Unknown(value.to_string())
    }
}

impl From<std::io::Error> for AssetIoError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(Arc::new(value))
    }
}

impl From<std::io::ErrorKind> for AssetIoError {
    fn from(value: std::io::ErrorKind) -> Self {
        Self::Io(Arc::new(std::io::Error::from(value)))
    }
}

impl From<&Path> for AssetIoError {
    fn from(value: &Path) -> Self {
        Self::NotFound(value.to_path_buf())
    }
}

impl From<bincode::error::EncodeError> for AssetIoError {
    fn from(value: bincode::error::EncodeError) -> Self {
        let error = std::io::Error::new(std::io::ErrorKind::InvalidData, value);
        Self::Io(Arc::new(error))
    }
}

impl From<bincode::error::DecodeError> for AssetIoError {
    fn from(value: bincode::error::DecodeError) -> Self {
        let error = std::io::Error::new(std::io::ErrorKind::InvalidData, value);
        Self::Io(Arc::new(error))
    }
}

impl From<ron::Error> for AssetIoError {
    fn from(value: ron::Error) -> Self {
        let error = std::io::Error::new(std::io::ErrorKind::InvalidData, value);
        Self::Io(Arc::new(error))
    }
}

impl From<ron::error::SpannedError> for AssetIoError {
    fn from(value: ron::error::SpannedError) -> Self {
        let error = std::io::Error::new(std::io::ErrorKind::InvalidData, value);
        Self::Io(Arc::new(error))
    }
}

pub type BoxedFuture<'a, T, E = AssetIoError> = Box<dyn Future<Output = Result<T, E>> + Send + 'a>;
pub type AssetFuture<'a, T, E = AssetIoError> = Pin<BoxedFuture<'a, T, E>>;

pub trait AsyncReader: AsyncRead + Send + Sync + Unpin {
    fn read_to_end<'a>(&'a mut self, buf: &'a mut Vec<u8>) -> AssetFuture<'a, usize>;
}

impl AsyncReader for Box<dyn AsyncReader> {
    fn read_to_end<'a>(&'a mut self, buf: &'a mut Vec<u8>) -> AssetFuture<'a, usize> {
        self.as_mut().read_to_end(buf)
    }
}

pub trait AsyncWriter: AsyncWrite + Send + Sync + Unpin {}

pub trait PathStream: Stream<Item = PathBuf> + Send + Unpin {}

impl<T: Stream<Item = PathBuf> + Send + Unpin> PathStream for T {}

pub trait FileSystem: Send + Sync + 'static {
    type Reader: AsyncReader;
    type Writer: AsyncWriter;

    fn root(&self) -> &Path;
    fn reader(
        &self,
        path: &Path,
    ) -> impl Future<Output = Result<Self::Reader, AssetIoError>> + Send;
    fn read_dir(
        &self,
        path: &Path,
    ) -> impl Future<Output = Result<Box<dyn PathStream>, AssetIoError>> + Send;
    fn is_dir(&self, path: &Path) -> impl Future<Output = Result<bool, AssetIoError>> + Send;
    fn writer(
        &self,
        path: &Path,
    ) -> impl Future<Output = Result<Self::Writer, AssetIoError>> + Send;
    fn create_dir(&self, path: &Path) -> impl Future<Output = Result<(), AssetIoError>> + Send;
    fn create_dir_all(&self, path: &Path) -> impl Future<Output = Result<(), AssetIoError>> + Send;
    fn rename(
        &self,
        from: &Path,
        to: &Path,
    ) -> impl Future<Output = Result<(), AssetIoError>> + Send;
    fn remove(&self, path: &Path) -> impl Future<Output = Result<(), AssetIoError>> + Send;
    fn remove_dir(&self, path: &Path) -> impl Future<Output = Result<(), AssetIoError>> + Send;
    fn exists(&self, path: &Path) -> impl Future<Output = Result<bool, AssetIoError>> + Send;
}

pub trait ErasedFileSystem: downcast_rs::Downcast + Send + Sync + 'static {
    fn root(&self) -> &Path;
    fn reader<'a>(&'a self, path: &'a Path) -> AssetFuture<'a, Box<dyn AsyncReader>>;
    fn read_dir<'a>(&'a self, path: &'a Path) -> AssetFuture<'a, Box<dyn PathStream>>;
    fn is_dir<'a>(&'a self, path: &'a Path) -> AssetFuture<'a, bool>;
    fn writer<'a>(&'a self, path: &'a Path) -> AssetFuture<'a, Box<dyn AsyncWriter>>;
    fn create_dir<'a>(&'a self, path: &'a Path) -> AssetFuture<'a, ()>;
    fn create_dir_all<'a>(&'a self, path: &'a Path) -> AssetFuture<'a, ()>;
    fn rename<'a>(&'a self, from: &'a Path, to: &'a Path) -> AssetFuture<'a, ()>;
    fn remove<'a>(&'a self, path: &'a Path) -> AssetFuture<'a, ()>;
    fn remove_dir<'a>(&'a self, path: &'a Path) -> AssetFuture<'a, ()>;
    fn exists<'a>(&'a self, path: &'a Path) -> AssetFuture<'a, bool>;
}

impl<T: FileSystem> ErasedFileSystem for T {
    fn root(&self) -> &Path {
        FileSystem::root(self)
    }

    fn reader<'a>(&'a self, path: &'a Path) -> AssetFuture<'a, Box<dyn AsyncReader>> {
        Box::pin(async {
            let reader = self.reader(path).await?;
            Ok(Box::new(reader) as Box<dyn AsyncReader>)
        })
    }

    fn read_dir<'a>(&'a self, path: &'a Path) -> AssetFuture<'a, Box<dyn PathStream>> {
        Box::pin(async { self.read_dir(path).await })
    }

    fn is_dir<'a>(&'a self, path: &'a Path) -> AssetFuture<'a, bool> {
        Box::pin(async { self.is_dir(path).await })
    }

    fn writer<'a>(&'a self, path: &'a Path) -> AssetFuture<'a, Box<dyn AsyncWriter>> {
        Box::pin(async {
            let writer = self.writer(path).await?;
            Ok(Box::new(writer) as Box<dyn AsyncWriter>)
        })
    }

    fn create_dir<'a>(&'a self, path: &'a Path) -> AssetFuture<'a, ()> {
        Box::pin(async { self.create_dir(path).await })
    }

    fn create_dir_all<'a>(&'a self, path: &'a Path) -> AssetFuture<'a, ()> {
        Box::pin(async { self.create_dir_all(path).await })
    }

    fn rename<'a>(&'a self, from: &'a Path, to: &'a Path) -> AssetFuture<'a, ()> {
        Box::pin(async { self.rename(from, to).await })
    }

    fn remove<'a>(&'a self, path: &'a Path) -> AssetFuture<'a, ()> {
        Box::pin(async { self.remove(path).await })
    }

    fn remove_dir<'a>(&'a self, path: &'a Path) -> AssetFuture<'a, ()> {
        Box::pin(async { self.remove_dir(path).await })
    }

    fn exists<'a>(&'a self, path: &'a Path) -> AssetFuture<'a, bool> {
        Box::pin(async { self.exists(path).await })
    }
}

pub trait PathExt {
    fn ext(&self) -> Option<&str>;
    fn append_ext(&self, ext: &str) -> PathBuf;
    fn with_prefix(&self, prefix: impl AsRef<Path>) -> Cow<Path>;
    fn without_prefix(&self, prefix: impl AsRef<Path>) -> &Path;
}

impl<T: AsRef<Path>> PathExt for T {
    fn ext(&self) -> Option<&str> {
        self.as_ref().extension().and_then(|ext| ext.to_str())
    }
    fn append_ext(&self, ext: &str) -> PathBuf {
        let path = self.as_ref().to_path_buf();
        format!("{}.{}", path.display(), ext).into()
    }

    fn with_prefix(&self, prefix: impl AsRef<Path>) -> Cow<Path> {
        match self.as_ref().starts_with(prefix.as_ref()) {
            false => Cow::Owned(prefix.as_ref().join(self)),
            true => Cow::Borrowed(self.as_ref()),
        }
    }

    fn without_prefix(&self, prefix: impl AsRef<Path>) -> &Path {
        let path = self.as_ref();
        let prefix = prefix.as_ref();
        path.strip_prefix(prefix).unwrap_or(path)
    }
}

pub fn deserialize<T: for<'a> Deserialize<'a>>(data: &[u8]) -> Result<T, AssetIoError> {
    bincode::serde::decode_from_slice::<T, _>(data, bincode::config::standard())
        .map(|v| v.0)
        .map_err(AssetIoError::from)
}

pub fn serialize<T: Serialize>(value: &T) -> Result<Vec<u8>, AssetIoError> {
    bincode::serde::encode_to_vec(value, bincode::config::standard()).map_err(AssetIoError::from)
}
