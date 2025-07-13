use smol::{
    io::{AsyncRead, AsyncSeek, AsyncWrite},
    stream::Stream,
};
use std::{
    path::{Path, PathBuf},
    pin::Pin,
    sync::Arc,
};
use thiserror::Error;

pub mod embedded;
pub mod local;
pub mod path;
pub mod vfs;

pub use embedded::*;
pub use local::*;
pub use path::*;
pub use vfs::*;

#[derive(Debug, Clone, Error)]
pub enum AsyncIoError {
    #[error("Path not found: {0}")]
    NotFound(PathBuf),

    #[error("{0}")]
    Io(Arc<std::io::Error>),

    #[error("Http error {0}")]
    Http(u16),

    #[error("{0}")]
    Unknown(String),
}

impl AsyncIoError {
    pub fn unknown(value: impl ToString) -> Self {
        Self::Unknown(value.to_string())
    }

    pub fn invalid_data() -> Self {
        std::io::ErrorKind::InvalidData.into()
    }
}

impl From<std::io::Error> for AsyncIoError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(Arc::new(value))
    }
}

impl From<std::io::ErrorKind> for AsyncIoError {
    fn from(value: std::io::ErrorKind) -> Self {
        Self::Io(Arc::new(std::io::Error::from(value)))
    }
}

impl From<&Path> for AsyncIoError {
    fn from(value: &Path) -> Self {
        Self::NotFound(value.to_path_buf())
    }
}

impl From<bincode::error::EncodeError> for AsyncIoError {
    fn from(value: bincode::error::EncodeError) -> Self {
        let error = std::io::Error::new(std::io::ErrorKind::InvalidData, value);
        Self::Io(Arc::new(error))
    }
}

impl From<bincode::error::DecodeError> for AsyncIoError {
    fn from(value: bincode::error::DecodeError) -> Self {
        let error = std::io::Error::new(std::io::ErrorKind::InvalidData, value);
        Self::Io(Arc::new(error))
    }
}

pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

pub trait AsyncReader: AsyncRead + AsyncSeek + Send + Sync + Unpin {
    fn read_to_end<'a>(
        &'a mut self,
        buf: &'a mut Vec<u8>,
    ) -> BoxFuture<'a, Result<usize, AsyncIoError>>;
}

impl AsyncReader for Box<dyn AsyncReader> {
    fn read_to_end<'a>(
        &'a mut self,
        buf: &'a mut Vec<u8>,
    ) -> BoxFuture<'a, Result<usize, AsyncIoError>> {
        self.as_mut().read_to_end(buf)
    }
}

pub trait AsyncWriter: AsyncWrite + Send + Sync + Unpin {
    fn flush<'a>(&'a mut self) -> BoxFuture<'a, Result<(), AsyncIoError>>;
}

impl AsyncWriter for Box<dyn AsyncWriter> {
    fn flush<'a>(&'a mut self) -> BoxFuture<'a, Result<(), AsyncIoError>> {
        self.as_mut().flush()
    }
}

pub trait PathStream: Stream<Item = PathBuf> + Send + Unpin {}

impl<T: Stream<Item = PathBuf> + Send + Unpin> PathStream for T {}

pub trait FileSystem: Send + Sync + 'static {
    type Reader: AsyncReader;
    type Writer: AsyncWriter;

    fn root(&self) -> &Path;
    fn reader(
        &self,
        path: &Path,
    ) -> impl Future<Output = Result<Self::Reader, AsyncIoError>> + Send;
    fn read_dir(
        &self,
        path: &Path,
    ) -> impl Future<Output = Result<Box<dyn PathStream>, AsyncIoError>> + Send;
    fn is_dir(&self, path: &Path) -> impl Future<Output = Result<bool, AsyncIoError>> + Send;
    fn writer(
        &self,
        path: &Path,
    ) -> impl Future<Output = Result<Self::Writer, AsyncIoError>> + Send;
    fn create_dir(&self, path: &Path) -> impl Future<Output = Result<(), AsyncIoError>> + Send;
    fn create_dir_all(&self, path: &Path) -> impl Future<Output = Result<(), AsyncIoError>> + Send;
    fn rename(
        &self,
        from: &Path,
        to: &Path,
    ) -> impl Future<Output = Result<(), AsyncIoError>> + Send;
    fn remove(&self, path: &Path) -> impl Future<Output = Result<(), AsyncIoError>> + Send;
    fn remove_dir(&self, path: &Path) -> impl Future<Output = Result<(), AsyncIoError>> + Send;
    fn exists(&self, path: &Path) -> impl Future<Output = Result<bool, AsyncIoError>> + Send;
}

pub trait ErasedFileSystem: Send + Sync + 'static {
    fn root(&self) -> &Path;
    fn reader<'a>(
        &'a self,
        path: &'a Path,
    ) -> BoxFuture<'a, Result<Box<dyn AsyncReader>, AsyncIoError>>;
    fn read_dir<'a>(
        &'a self,
        path: &'a Path,
    ) -> BoxFuture<'a, Result<Box<dyn PathStream>, AsyncIoError>>;
    fn is_dir<'a>(&'a self, path: &'a Path) -> BoxFuture<'a, Result<bool, AsyncIoError>>;
    fn writer<'a>(
        &'a self,
        path: &'a Path,
    ) -> BoxFuture<'a, Result<Box<dyn AsyncWriter>, AsyncIoError>>;
    fn create_dir<'a>(&'a self, path: &'a Path) -> BoxFuture<'a, Result<(), AsyncIoError>>;
    fn create_dir_all<'a>(&'a self, path: &'a Path) -> BoxFuture<'a, Result<(), AsyncIoError>>;
    fn rename<'a>(
        &'a self,
        from: &'a Path,
        to: &'a Path,
    ) -> BoxFuture<'a, Result<(), AsyncIoError>>;
    fn remove<'a>(&'a self, path: &'a Path) -> BoxFuture<'a, Result<(), AsyncIoError>>;
    fn remove_dir<'a>(&'a self, path: &'a Path) -> BoxFuture<'a, Result<(), AsyncIoError>>;
    fn exists<'a>(&'a self, path: &'a Path) -> BoxFuture<'a, Result<bool, AsyncIoError>>;
}

impl<T: FileSystem> ErasedFileSystem for T {
    fn root(&self) -> &Path {
        FileSystem::root(self)
    }

    fn reader<'a>(
        &'a self,
        path: &'a Path,
    ) -> BoxFuture<'a, Result<Box<dyn AsyncReader>, AsyncIoError>> {
        Box::pin(async {
            let reader = self.reader(path).await?;
            Ok(Box::new(reader) as Box<dyn AsyncReader>)
        })
    }

    fn read_dir<'a>(
        &'a self,
        path: &'a Path,
    ) -> BoxFuture<'a, Result<Box<dyn PathStream>, AsyncIoError>> {
        Box::pin(async { self.read_dir(path).await })
    }

    fn is_dir<'a>(&'a self, path: &'a Path) -> BoxFuture<'a, Result<bool, AsyncIoError>> {
        Box::pin(async { self.is_dir(path).await })
    }

    fn writer<'a>(
        &'a self,
        path: &'a Path,
    ) -> BoxFuture<'a, Result<Box<dyn AsyncWriter>, AsyncIoError>> {
        Box::pin(async {
            let writer = self.writer(path).await?;
            Ok(Box::new(writer) as Box<dyn AsyncWriter>)
        })
    }

    fn create_dir<'a>(&'a self, path: &'a Path) -> BoxFuture<'a, Result<(), AsyncIoError>> {
        Box::pin(async { self.create_dir(path).await })
    }

    fn create_dir_all<'a>(&'a self, path: &'a Path) -> BoxFuture<'a, Result<(), AsyncIoError>> {
        Box::pin(async { self.create_dir_all(path).await })
    }

    fn rename<'a>(
        &'a self,
        from: &'a Path,
        to: &'a Path,
    ) -> BoxFuture<'a, Result<(), AsyncIoError>> {
        Box::pin(async { self.rename(from, to).await })
    }

    fn remove<'a>(&'a self, path: &'a Path) -> BoxFuture<'a, Result<(), AsyncIoError>> {
        Box::pin(async { self.remove(path).await })
    }

    fn remove_dir<'a>(&'a self, path: &'a Path) -> BoxFuture<'a, Result<(), AsyncIoError>> {
        Box::pin(async { self.remove_dir(path).await })
    }

    fn exists<'a>(&'a self, path: &'a Path) -> BoxFuture<'a, Result<bool, AsyncIoError>> {
        Box::pin(async { self.exists(path).await })
    }
}
