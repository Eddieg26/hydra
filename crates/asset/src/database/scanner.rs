use std::collections::HashSet;

use crate::{
    database::{
        AssetDatabase,
        config::{
            cache::{ArtifactPath, Checksum},
            sources::{AssetFileSource, AssetFileSystem},
        },
    },
    ext::PathExt,
    io::{
        AsyncIoError,
        path::{AssetPath, AssetSource},
    },
    settings::Settings,
};
use serde::{Deserialize, Serialize};
use smol::stream::StreamExt;

#[derive(Debug, Clone, thiserror::Error)]
pub enum AssetScanError {
    #[error("Failed to scan asset source: {source}, error: {error}")]
    Source {
        source: AssetSource<'static>,
        error: AsyncIoError,
    },

    #[error("Failed to scan asset file: {path}, error: {error}")]
    Dir {
        path: AssetPath<'static>,
        error: AsyncIoError,
    },

    #[error("Failed to scan asset file: {path}, error: {error}")]
    Unknown {
        path: AssetPath<'static>,
        error: AsyncIoError,
    },
}

impl AssetScanError {
    pub fn path(&self) -> Option<&AssetPath<'static>> {
        match self {
            AssetScanError::Source { .. } => None,
            AssetScanError::Dir { path, .. } => Some(path),
            AssetScanError::Unknown { path, .. } => Some(path),
        }
    }
}

pub type Blacklist = HashSet<AssetPath<'static>>;

#[derive(Debug, Default)]
pub struct ScanInfo {
    pub paths: Vec<AssetPath<'static>>,
    pub removed: Vec<AssetPath<'static>>,
}

impl ScanInfo {
    fn with_paths(mut self, paths: Vec<AssetPath<'static>>) -> Self {
        self.paths = paths;
        self
    }

    fn extend(&mut self, other: Self) {
        self.paths.extend(other.paths);
        self.removed.extend(other.removed);
    }

    pub fn is_empty(&self) -> bool {
        self.paths.is_empty() && self.removed.is_empty()
    }
}

impl AssetDatabase {
    pub async fn scan(&self, blacklist: &Blacklist) -> Result<ScanInfo, AssetScanError> {
        let mut info = ScanInfo::default();
        for source in self.config.sources().iter() {
            let result = self.scan_source(source, blacklist).await?;
            info.extend(result);
        }

        Ok(info)
    }

    async fn scan_source(
        &self,
        source: AssetFileSource<'_>,
        blacklist: &Blacklist,
    ) -> Result<ScanInfo, AssetScanError> {
        let path = AssetPath::new(source.name.clone(), "");
        let mut info = self.scan_dir(path, &source).await?;
        let mut imports = Vec::new();

        while let Some(path) = info.paths.pop() {
            match source.is_dir(&path).await {
                Ok(true) => match self.scan_dir(path, &source).await {
                    Ok(other) => info.extend(other),
                    Err(error) => self.send_event(error).await,
                },
                Ok(false) => {
                    if blacklist.contains(&path) {
                        continue;
                    }

                    let Some(ext) = path.ext() else {
                        continue;
                    };

                    if ext == "meta" || self.config.importers().get_by_ext(ext).is_none() {
                        continue;
                    }

                    match self.scan_file(path, &source).await {
                        Ok(Some(path)) => imports.push(path),
                        Ok(None) => continue,
                        Err(error) => self.send_event(error).await,
                    }
                }
                Err(error) => {
                    self.send_event(AssetScanError::Unknown { path, error })
                        .await
                }
            }
        }

        Ok(info.with_paths(imports))
    }

    async fn scan_dir(
        &self,
        path: AssetPath<'static>,
        fs: &AssetFileSystem,
    ) -> Result<ScanInfo, AssetScanError> {
        let mut paths = match fs.read_dir(&path).await {
            Ok(stream) => stream.collect::<Vec<_>>().await,
            Err(error) => return Err(AssetScanError::Dir { path, error }),
        };

        let mut folder = fs.load_settings::<Folder>(&path).await.unwrap_or_default();

        let paths = paths
            .drain(..)
            .map(|c| AssetPath::new_owned(path.source().clone(), c))
            .collect::<Vec<_>>();

        let removed = folder
            .items
            .drain(..)
            .filter_map(|p| (!paths.contains(&p)).then_some(p))
            .collect();

        folder.items = paths.clone();

        let _ = fs.save_settings(&path, &folder).await;

        Ok(ScanInfo { paths, removed })
    }

    async fn scan_file(
        &self,
        path: AssetPath<'static>,
        fs: &AssetFileSystem,
    ) -> Result<Option<AssetPath<'static>>, AssetScanError> {
        let library = self.library.read().await;

        let Some(id) = library.get(&path) else {
            return Ok(Some(path));
        };

        let settings_path = path.append_ext("meta");
        if !fs.exists(&settings_path).await.unwrap_or_default() {
            return Ok(Some(path));
        }

        let Ok(artifact) = self.cache().artifact_reader(ArtifactPath::Cache, id).await else {
            return Ok(Some(path));
        };

        let Ok(meta) = artifact.read_metadata().await else {
            return Ok(Some(path));
        };

        let Ok(settings) = fs.read(&settings_path).await else {
            return Ok(Some(path));
        };

        let Ok(asset) = fs.read(&path).await else {
            return Ok(Some(path));
        };

        let checksum = Checksum::from(&asset, &settings);

        if checksum != meta.import.checksum {
            return Ok(Some(path));
        }

        let Some(full_checksum) = self.cache().get_full_checksum(checksum, &meta).await else {
            return Ok(Some(path));
        };

        if full_checksum != meta.import.full_checksum {
            return Ok(Some(path));
        }

        Ok(None)
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Folder {
    items: Vec<AssetPath<'static>>,
}

impl Settings for Folder {}
