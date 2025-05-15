use super::{AssetIoError, AyncReader, AsyncWriter, FileSystem, PathExt};
use futures::{executor::block_on, AsyncRead, AsyncWrite, StreamExt};
use smol::lock::RwLock;
use std::{
    collections::HashMap, io::{Cursor, Read, Seek, Write}, path::{Path, PathBuf}, sync::Arc, time::SystemTime
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VirtualEntryType {
    File,
    Directory,
}

pub struct VirtualEntry {
    ty: VirtualEntryType,
    data: Arc<Vec<u8>>,
    created: SystemTime,
}

impl VirtualEntry {
    fn file(data: Vec<u8>, created: Option<SystemTime>) -> Self {
        Self {
            ty: VirtualEntryType::File,
            data: Arc::new(data),
            created: created.unwrap_or(SystemTime::now()),
        }
    }

    fn directory() -> Self {
        Self {
            ty: VirtualEntryType::Directory,
            data: Arc::default(),
            created: SystemTime::now(),
        }
    }
}

pub struct FileReader {
    data: Arc<Vec<u8>>,
    position: u64,
}

impl FileReader {
    pub fn size(&self) -> u64 {
        self.data.len() as u64
    }
}

impl Read for FileReader {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let len = buf.len().min((self.size() - self.position) as usize);
        if len > 0 {
            buf[..len]
                .copy_from_slice(&self.data[self.position as usize..self.position as usize + len]);
        }

        self.position += len as u64;
        Ok(len)
    }
}

impl Seek for FileReader {
    fn seek(&mut self, pos: std::io::SeekFrom) -> std::io::Result<u64> {
        match pos {
            std::io::SeekFrom::Start(offset) => self.position = offset,
            std::io::SeekFrom::End(offset) => {
                self.position = (self.position as i64 + offset) as u64
            }
            std::io::SeekFrom::Current(offset) => {
                self.position = (self.data.len() as i64 + offset) as u64
            }
        }

        Ok(self.position)
    }
}

impl AsyncRead for FileReader {
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        _: &mut std::task::Context<'_>,
        buf: &mut [u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        let len = buf.len().min((self.size() - self.position) as usize);
        if len > 0 {
            buf[..len]
                .copy_from_slice(&self.data[self.position as usize..self.position as usize + len]);
        }

        self.position += len as u64;
        std::task::Poll::Ready(Ok(len))
    }
}

impl AyncReader for FileReader {
    fn read_to_end<'a>(&'a mut self, buf: &'a mut Vec<u8>) -> super::AssetFuture<'a, usize> {
        Box::pin(async move {
            let len = self.data.len();
            if self.position < len as u64 {
                buf.extend(&self.data[self.position as usize..]);
                let end = len - self.position as usize;
                Ok(end)
            } else {
                Ok(0)
            }
        })
    }
}

pub struct FileWriter {
    data: Cursor<Vec<u8>>,
    path: PathBuf,
    fs: Handle,
}

impl Write for FileWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.data.write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.data.flush()
    }
}

impl Seek for FileWriter {
    fn seek(&mut self, pos: std::io::SeekFrom) -> std::io::Result<u64> {
        self.data.seek(pos)
    }
}

impl AsyncWrite for FileWriter {
    fn poll_write(
        mut self: std::pin::Pin<&mut Self>,
        _: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        let size = self.data.write(buf)?;
        std::task::Poll::Ready(Ok(size))
    }

    fn poll_flush(
        mut self: std::pin::Pin<&mut Self>,
        _: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        self.data.flush()?;
        std::task::Poll::Ready(Ok(()))
    }

    fn poll_close(
        self: std::pin::Pin<&mut Self>,
        _: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        std::task::Poll::Ready(Ok(()))
    }
}

impl AsyncWriter for FileWriter {}

impl Drop for FileWriter {
    fn drop(&mut self) {
        let mut fs = self.fs.write_arc_blocking();
        let prev = fs.entries.remove(&self.path);
        let data = std::mem::take(self.data.get_mut());

        let file = VirtualEntry::file(data, prev.map(|p| p.created));
        fs.entries.insert(self.path.clone(), file);
    }
}

#[derive(Default)]
pub struct VirtualAssets {
    entries: HashMap<PathBuf, VirtualEntry>,
}

type Handle = Arc<RwLock<VirtualAssets>>;

#[derive(Clone)]
pub struct VirtualFs {
    root: Box<Path>,
    fs: Handle,
}

impl VirtualFs {
    pub fn new(root: impl AsRef<Path>) -> Self {
        let mut fs = VirtualAssets::default();
        fs.entries
            .insert(root.as_ref().to_path_buf(), VirtualEntry::directory());

        Self {
            root: root.as_ref().to_path_buf().into_boxed_path(),
            fs: Arc::new(RwLock::new(fs)),
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn print(&self, path: &Path, depth: usize, f: &mut std::fmt::Formatter<'_>) {
        if let Ok(mut paths) = block_on(self.read_dir(path)) {
            let spaces = " ".repeat(depth);
            writeln!(f, "{}|--{}", spaces, path.display()).unwrap();

            while let Some(path) = block_on(paths.next()) {
                let is_dir = block_on(self.is_dir(&path)).unwrap_or(false);
                match is_dir {
                    true => self.print(&path, depth + 2, f),
                    false => {
                        let spaces = match depth {
                            0 => " ".repeat(depth + 2),
                            _ => " ".repeat(depth + 3),
                        };
                        let dashes = "-".repeat(2);
                        writeln!(f, "{}|{}{}", spaces, dashes, path.display()).unwrap();
                    }
                }
            }
        }
    }
}

impl std::fmt::Display for VirtualFs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.print(&self.root, 0, f);
        Ok(())
    }
}

impl FileSystem for VirtualFs {
    type Reader = FileReader;
    type Writer = FileWriter;

    fn root(&self) -> &Path {
        &self.root
    }

    async fn reader(&self, path: &std::path::Path) -> Result<Self::Reader, super::AssetIoError> {
        let path = path.with_prefix(&self.root);
        let fs = self.fs.read().await;
        match fs.entries.get(path.as_ref()) {
            Some(entry) => match entry.ty {
                VirtualEntryType::File => {
                    let reader = FileReader {
                        data: entry.data.clone(),
                        position: 0,
                    };
                    Ok(reader)
                }
                VirtualEntryType::Directory => {
                    Err(AssetIoError::from(std::io::ErrorKind::Unsupported))
                }
            },
            None => Err(AssetIoError::NotFound(path.to_path_buf())),
        }
    }

    async fn read_dir(
        &self,
        path: &std::path::Path,
    ) -> Result<Box<dyn super::PathStream>, super::AssetIoError> {
        let path = path.with_prefix(&self.root);
        let fs = self.fs.read().await;
        match fs.entries.get(path.as_ref()) {
            Some(entry) => match entry.ty {
                VirtualEntryType::File => Err(AssetIoError::from(std::io::ErrorKind::Unsupported)),
                VirtualEntryType::Directory => {
                    let mut paths: Vec<PathBuf> = vec![];
                    let prefix = format!("{}/", path.display());

                    for entry in fs.entries.keys() {
                        if entry.as_path() == path {
                            continue;
                        }

                        if prefix.len() == 1 && entry.parent() == Some(path.as_ref()) {
                            paths.push(entry.to_path_buf());
                            continue;
                        }

                        if let Ok(path) = entry.strip_prefix(&prefix) {
                            if path.components().count() == 1 {
                                paths.push(entry.to_path_buf());
                            }
                        }
                    }

                    paths.sort();

                    Ok(Box::new(futures::stream::iter(paths)))
                }
            },
            None => Err(AssetIoError::NotFound(path.to_path_buf())),
        }
    }

    async fn is_dir(&self, path: &std::path::Path) -> Result<bool, super::AssetIoError> {
        let path = path.with_prefix(&self.root);
        let fs = self.fs.read().await;
        match fs
            .entries
            .get(path.as_ref())
            .map(|e| e.ty == VirtualEntryType::Directory)
        {
            Some(value) => Ok(value),
            None => Err(AssetIoError::NotFound(path.to_path_buf())),
        }
    }

    async fn writer(&self, path: &std::path::Path) -> Result<Self::Writer, super::AssetIoError> {
        let path = path.with_prefix(&self.root);
        let fs = self.fs.read().await;
        match fs.entries.get(path.as_ref()) {
            Some(entry) => match entry.ty {
                VirtualEntryType::File => {
                    let writer = FileWriter {
                        data: Cursor::new(vec![]),
                        path: path.to_path_buf(),
                        fs: self.fs.clone(),
                    };
                    Ok(writer)
                }
                VirtualEntryType::Directory => {
                    Err(AssetIoError::from(std::io::ErrorKind::Unsupported))
                }
            },
            None => {
                let writer = FileWriter {
                    data: Cursor::new(vec![]),
                    path: path.to_path_buf(),
                    fs: self.fs.clone(),
                };
                Ok(writer)
            }
        }
    }

    async fn create_dir(&self, path: &std::path::Path) -> Result<(), super::AssetIoError> {
        let path = path.with_prefix(&self.root);
        let mut fs = self.fs.write().await;
        match fs.entries.get(path.as_ref()) {
            Some(entry) => match entry.ty {
                VirtualEntryType::File => {
                    Err(AssetIoError::from(std::io::ErrorKind::AlreadyExists))
                }
                VirtualEntryType::Directory => Ok(()),
            },
            None => {
                if let Some(parent) = path.parent() {
                    if fs.entries.get(parent).is_none() {
                        return Err(AssetIoError::NotFound(parent.to_path_buf()));
                    }
                }

                let dir = VirtualEntry::directory();
                fs.entries.insert(path.to_path_buf(), dir);

                Ok(())
            }
        }
    }

    async fn create_dir_all(&self, path: &std::path::Path) -> Result<(), super::AssetIoError> {
        let path = path.with_prefix(&self.root);
        let mut fs = self.fs.write().await;
        let mut current = path.to_path_buf();
        while let Some(parent) = current.parent() {
            match fs.entries.get(parent).and_then(|e| Some(e.ty)) {
                Some(VirtualEntryType::File) => {
                    return Err(AssetIoError::from(std::io::Error::new(
                        std::io::ErrorKind::Unsupported,
                        format!("Not a directory: {:?}", parent),
                    )))
                }
                Some(VirtualEntryType::Directory) => current = parent.to_path_buf(),
                None => {
                    let dir = VirtualEntry::directory();
                    fs.entries.insert(parent.to_path_buf(), dir);
                    current = parent.to_path_buf();
                }
            }
        }

        std::mem::drop(fs);

        self.create_dir(path.as_ref()).await
    }

    async fn rename(
        &self,
        from: &std::path::Path,
        to: &std::path::Path,
    ) -> Result<(), super::AssetIoError> {
        let from = from.with_prefix(&self.root);
        let to = to.with_prefix(&self.root);
        let mut fs = self.fs.write().await;
        match fs.entries.remove(from.as_ref()) {
            Some(entry) => match fs.entries.contains_key(to.as_ref()) {
                true => return Err(AssetIoError::from(std::io::ErrorKind::AlreadyExists)),
                false => {
                    fs.entries.insert(to.to_path_buf(), entry);
                    Ok(())
                }
            },
            None => Err(AssetIoError::NotFound(from.to_path_buf())),
        }
    }

    async fn remove(&self, path: &std::path::Path) -> Result<(), super::AssetIoError> {
        let path = path.with_prefix(&self.root);
        let mut fs = self.fs.write().await;
        match fs.entries.remove(path.as_ref()) {
            Some(_) => Ok(()),
            None => Err(AssetIoError::NotFound(path.to_path_buf())),
        }
    }

    async fn remove_dir(&self, path: &std::path::Path) -> Result<(), super::AssetIoError> {
        let path = path.with_prefix(&self.root);
        let mut fs = self.fs.write().await;
        match fs.entries.get(path.as_ref()).map(|e| e.ty) {
            Some(VirtualEntryType::File) => Err(AssetIoError::from(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                format!("Not a directory: {:?}", path),
            ))),
            Some(VirtualEntryType::Directory) => {
                Ok(fs.entries.remove(path.as_ref()).map(|_| ()).unwrap())
            }
            None => Err(AssetIoError::NotFound(path.to_path_buf())),
        }
    }

    async fn exists(&self, path: &std::path::Path) -> Result<bool, super::AssetIoError> {
        let fs = self.fs.read().await;
        Ok(fs.entries.contains_key(path))
    }
}
