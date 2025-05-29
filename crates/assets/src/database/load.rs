use super::{AssetDatabase, DatabaseEvent, state::LoadState};
use crate::{
    asset::{Asset, AssetId, ErasedAsset, ErasedId},
    config::AssetConfig,
    io::{ArtifactMeta, AssetIoError, AssetPath},
};
use ecs::core::task::IoTaskPool;
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

        self.spawn_load_task(id);

        Ok(id)
    }

    fn spawn_load_task(&self, id: ErasedId) {
        IoTaskPool::get()
            .spawn(async move {
                let AssetDatabase {
                    config,
                    states,
                    sender,
                    ..
                } = AssetDatabase::get();

                let mut ids = vec![id];

                while let Some(id) = ids.pop() {
                    let mut states = states.write().await;

                    if states.get_load_state(id).is_loading() {
                        continue;
                    }

                    states.loading(id);

                    let (asset, meta) = match Self::load_internal(id, &config).await {
                        Ok(result) => result,
                        Err(error) => {
                            let _ = sender.send(DatabaseEvent::LoadError(error));
                            states.failed(id);
                            continue;
                        }
                    };

                    let loaded = states.loaded(&meta);

                    for dependency in &meta.dependencies {
                        if matches!(
                            states.get_load_state(*dependency),
                            LoadState::Unloaded | LoadState::Failed
                        ) {
                            ids.push(*dependency);
                        }
                    }

                    if let Some(parent) = meta.parent.and_then(|p| {
                        let state = states.get_load_state(p);
                        (state.is_unloaded() || state.is_failed()).then_some(p)
                    }) {
                        ids.push(parent);
                    }

                    let _ = sender.send(DatabaseEvent::AssetLoaded { asset, meta });
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
