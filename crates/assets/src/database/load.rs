use super::{
    AssetDatabase,
    commands::{AssetCommand, LoadDependencies},
};
use crate::{
    asset::{Asset, AssetId, ErasedAsset, ErasedId},
    config::AssetConfig,
    io::{ArtifactMeta, AssetIoError, AssetPath},
};
use ecs::{Event, core::task::IoTaskPool};
use serde::{Deserialize, Serialize};

#[derive(thiserror::Error, Debug)]
pub enum AssetLoadError {
    #[error("Asset not found: {0}")]
    NotFound(LoadPath<'static>),
    #[error("Failed to load asset: {0}")]
    Io(AssetIoError),
    #[error("Asset not registered: {0}")]
    NotRegistered(LoadPath<'static>),
    #[error("Failed to deserialize asset: {0}")]
    Deserialize(AssetIoError),
}

impl From<LoadPath<'_>> for AssetLoadError {
    fn from(path: LoadPath<'_>) -> Self {
        AssetLoadError::NotFound(path.into_owned())
    }
}

impl From<AssetIoError> for AssetLoadError {
    fn from(error: AssetIoError) -> Self {
        AssetLoadError::Io(error)
    }
}

impl Event for AssetLoadError {}

impl AssetDatabase {
    pub fn load<'a, A: Asset + for<'de> Deserialize<'de>>(
        &self,
        path: impl Into<LoadPath<'a>>,
    ) -> Result<AssetId<A>, AssetLoadError> {
        self.load_erased(path).map(AssetId::from)
    }

    pub fn load_erased<'a>(
        &self,
        path: impl Into<LoadPath<'a>>,
    ) -> Result<ErasedId, AssetLoadError> {
        let load_path: LoadPath<'a> = path.into();
        let id = match load_path {
            LoadPath::Id(id) => id,
            LoadPath::Path(path) => {
                if let Some(id) = self.get_id(path.clone()) {
                    id
                } else {
                    return Err(AssetLoadError::NotFound(LoadPath::Path(path.into_owned())));
                }
            }
        };

        self.spawn_load_task(id, true);

        Ok(id)
    }

    pub fn reload<'a>(&self, path: impl Into<LoadPath<'a>>) {
        let load_path: LoadPath<'a> = path.into();
        let id = match load_path {
            LoadPath::Id(id) => id,
            LoadPath::Path(path) => match self.get_id(path) {
                Some(id) => id,
                None => return,
            },
        };

        self.spawn_load_task(id, false);
    }

    fn spawn_load_task(&self, id: ErasedId, load_dependencies: bool) {
        IoTaskPool::get()
            .spawn(async move {
                let db = AssetDatabase::get();

                let _writer = db.writer.read().await;

                let mut ids = vec![id];

                while let Some(id) = ids.pop() {
                    let mut states = db.states.write().await;

                    if states.get_load_state(id).is_loading() {
                        continue;
                    }

                    states.loading(id);

                    let (asset, meta) = match Self::load_internal(id, &db.config).await {
                        Ok(result) => result,
                        Err(error) => {
                            db.send_event(error).await;
                            states.failed(id);
                            continue;
                        }
                    };

                    if load_dependencies {
                        for dependency in meta.dependencies.iter().copied() {
                            if !states.get_load_state(dependency).is_loaded() {
                                ids.push(dependency);
                            }
                        }

                        if let Some(parent) = meta
                            .parent
                            .and_then(|p| (!states.get_load_state(p).is_loaded()).then_some(p))
                        {
                            ids.push(parent);
                        }
                    }

                    db.send_event(AssetCommand::add(
                        id,
                        meta.ty,
                        asset,
                        LoadDependencies::new(meta.parent, meta.dependencies),
                    ))
                    .await;
                }
            })
            .detach();
    }

    async fn load_internal(
        id: ErasedId,
        config: &AssetConfig,
    ) -> Result<(ErasedAsset, ArtifactMeta), AssetLoadError> {
        let artifact = config
            .cache()
            .get_artifact(id)
            .await
            .map_err(AssetLoadError::Io)?;

        let meta = config
            .registry()
            .get(artifact.ty())
            .ok_or(AssetLoadError::NotRegistered(LoadPath::Id(id)))?;

        let asset = match meta.deserialize(artifact.data()) {
            Some(Ok(asset)) => Ok(asset),
            Some(Err(error)) => Err(AssetLoadError::Deserialize(error.into())),
            None => Err(AssetLoadError::NotRegistered(LoadPath::Id(id))),
        }?;

        Ok((asset, artifact.meta))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LoadPath<'a> {
    Id(ErasedId),
    Path(AssetPath<'a>),
}

impl LoadPath<'_> {
    pub fn id(&self) -> Option<ErasedId> {
        match self {
            LoadPath::Id(id) => Some(*id),
            LoadPath::Path(_) => None,
        }
    }

    pub fn path(&self) -> Option<&AssetPath<'_>> {
        match self {
            LoadPath::Id(_) => None,
            LoadPath::Path(path) => Some(path),
        }
    }

    pub fn into_owned(self) -> LoadPath<'static> {
        match self {
            LoadPath::Id(id) => LoadPath::Id(id),
            LoadPath::Path(path) => LoadPath::Path(path.into_owned()),
        }
    }
}

impl<I: Into<ErasedId>> From<I> for LoadPath<'static> {
    fn from(value: I) -> Self {
        LoadPath::Id(value.into())
    }
}

impl<'a> From<AssetPath<'a>> for LoadPath<'a> {
    fn from(value: AssetPath<'a>) -> Self {
        LoadPath::Path(value)
    }
}

impl<'a> From<&'a str> for LoadPath<'a> {
    fn from(value: &'a str) -> Self {
        LoadPath::Path(AssetPath::from_str(value))
    }
}

impl From<String> for LoadPath<'static> {
    fn from(value: String) -> Self {
        LoadPath::Path(AssetPath::from_string(value))
    }
}

impl std::fmt::Display for LoadPath<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LoadPath::Id(id) => write!(f, "LoadPath(Id: {:?})", id.to_string()),
            LoadPath::Path(path) => write!(f, "LoadPath(Path: {})", path),
        }
    }
}
