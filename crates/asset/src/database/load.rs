use super::{AssetDatabase, commands::AssetCommand};
use crate::{
    asset::{Asset, AssetId, ErasedAsset, ErasedId},
    config::AssetConfig,
    io::{ArtifactMeta, AssetIoError, AssetPath},
};
use ecs::{Event, core::task::IoTaskPool};
use futures::TryFutureExt;
use serde::{Deserialize, Serialize};
use smol::lock::Mutex;
use std::sync::Arc;

#[derive(thiserror::Error, Debug, Clone, Event)]
pub enum LoadError {
    #[error("Asset not found: {0}")]
    NotFound(LoadPath<'static>),
    #[error("Failed to load asset: {0}")]
    Io(AssetIoError),
    #[error("Asset not registered: {0}")]
    NotRegistered(LoadPath<'static>),
    #[error("Failed to deserialize asset: {0}")]
    Deserialize(AssetIoError),
}

impl From<LoadPath<'_>> for LoadError {
    fn from(path: LoadPath<'_>) -> Self {
        LoadError::NotFound(path.into_owned())
    }
}

impl From<AssetIoError> for LoadError {
    fn from(error: AssetIoError) -> Self {
        LoadError::Io(error)
    }
}

impl AssetDatabase {
    pub fn load<A: Asset + for<'de> Deserialize<'de>>(
        &self,
        path: impl Into<LoadPath<'static>>,
    ) -> impl Future<Output = Result<AssetId<A>, LoadError>> {
        self.load_erased(path).map_ok(AssetId::from)
    }

    pub fn load_erased(
        &self,
        path: impl Into<LoadPath<'static>>,
    ) -> impl Future<Output = Result<ErasedId, LoadError>> {
        let load_path: LoadPath<'static> = path.into();

        self.spawn_load_task(load_path.clone())
    }

    pub fn reload(&self, path: impl Into<LoadPath<'static>>) {
        let load_path: LoadPath<'static> = path.into();

        IoTaskPool::get()
            .spawn(async move {
                let db = AssetDatabase::get();

                let _writer = db.writer.read().await;

                let Ok(id) = db.get_id_async(load_path).await else {
                    return;
                };

                if db.states.read().await.get_load_state(id).can_reload() {
                    match Self::load_internal(id, &db.config).await {
                        Ok((asset, meta)) => {
                            db.send_event(AssetCommand::add(id, meta.ty, asset, None))
                                .await;
                        }
                        Err(error) => {
                            db.send_event(error).await;
                            return;
                        }
                    };
                }
            })
            .detach();
    }

    fn spawn_load_task(
        &self,
        path: impl Into<LoadPath<'static>>,
    ) -> impl Future<Output = Result<ErasedId, LoadError>> {
        let load_path: LoadPath<'static> = path.into();

        let (sender, receiver) = futures::channel::oneshot::channel();
        let result = LoadTask::new(receiver);

        IoTaskPool::get()
            .spawn(async move {
                let db = AssetDatabase::get();

                if (*db.writer.read().await).is_some() {
                    db.import_inner().await;
                }

                let asset_id = match db.get_id_async(load_path).await {
                    Ok(id) => id,
                    Err(error) => {
                        let _ = sender.send(Err(error));
                        return;
                    }
                };

                let mut ids = vec![asset_id];
                let mut sender = Some(sender);

                while let Some(id) = ids.pop() {
                    let mut states = db.states.write().await;

                    if states.get_load_state(id).is_loading() {
                        continue;
                    }

                    states.loading(id);

                    let (asset, meta) = match Self::load_internal(id, &db.config).await {
                        Ok(result) => result,
                        Err(error) => {
                            db.send_event(error.clone()).await;
                            states.failed(id);
                            if let Some(sender) = sender.take() {
                                let _ = sender.send(Err(error));
                            }

                            continue;
                        }
                    };

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

                    db.send_event(AssetCommand::add(id, meta.ty, asset, None))
                        .await;

                    let loaded = states.loaded(
                        id,
                        meta.ty,
                        &meta.dependencies,
                        meta.parent,
                        meta.unload_action,
                    );

                    for (id, ty) in loaded {
                        db.send_event(AssetCommand::loaded(id, ty)).await;
                    }

                    if let Some(sender) = sender.take() {
                        let _ = sender.send(Ok(id));
                    }
                }
            })
            .detach();

        result
    }

    async fn load_internal(
        id: ErasedId,
        config: &AssetConfig,
    ) -> Result<(ErasedAsset, ArtifactMeta), LoadError> {
        let artifact = config
            .cache()
            .get_artifact(id)
            .await
            .map_err(LoadError::Io)?;

        let meta = config
            .registry()
            .get(artifact.ty())
            .ok_or(LoadError::NotRegistered(LoadPath::Id(id)))?;

        let asset = match meta.deserialize(artifact.data()) {
            Some(Ok(asset)) => Ok(asset),
            Some(Err(error)) => Err(LoadError::Deserialize(error.into())),
            None => Err(LoadError::NotRegistered(LoadPath::Id(id))),
        }?;

        Ok((asset, artifact.meta))
    }

    async fn get_id_async(&self, path: LoadPath<'static>) -> Result<ErasedId, LoadError> {
        match path {
            LoadPath::Id(id) => Ok(id),
            LoadPath::Path(path) => self
                .get_id(&path)
                .ok_or(LoadError::NotFound(LoadPath::Path(path.into_owned()))),
        }
    }
}

pub struct LoadTask {
    receiver: Arc<Mutex<Option<futures::channel::oneshot::Receiver<Result<ErasedId, LoadError>>>>>,
}

impl LoadTask {
    fn new(receiver: futures::channel::oneshot::Receiver<Result<ErasedId, LoadError>>) -> Self {
        Self {
            receiver: Arc::new(Mutex::new(Some(receiver))),
        }
    }
}

impl std::future::Future for LoadTask {
    type Output = Result<ErasedId, LoadError>;

    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        let mut receiver_lock = smol::block_on(self.receiver.lock());
        if let Some(receiver) = receiver_lock.as_mut() {
            match std::pin::Pin::new(receiver).poll(cx) {
                std::task::Poll::Ready(Ok(res)) => std::task::Poll::Ready(res),
                std::task::Poll::Ready(Err(_)) => std::task::Poll::Ready(Err(LoadError::Io(
                    AssetIoError::unknown("oneshot canceled"),
                ))),
                std::task::Poll::Pending => std::task::Poll::Pending,
            }
        } else {
            std::task::Poll::Pending
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Hash)]
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

#[allow(unused_imports, dead_code)]
mod tests {
    use std::path::PathBuf;

    use crate::{
        Asset, AssetId, AssetMetadata, DefaultSettings,
        config::{AssetConfigBuilder, importer::AssetImporter},
        database::AssetDatabase,
        io::{
            Artifact, ArtifactMeta, AssetCache, AssetIoError, AssetPath, FileSystem, ImportMeta,
            SourceName, VirtualFs, serialize,
        },
        plugin::AssetAppExt,
    };
    use ecs::{
        World,
        core::task::{IoTaskPool, TaskPool},
    };
    use serde::{Deserialize, Serialize};
    use smol::io::{AsyncReadExt, AsyncWriteExt};

    #[derive(Debug, PartialEq, Eq, Asset, Serialize, Deserialize)]
    pub struct Text(String);

    impl AssetImporter for Text {
        type Asset = Self;

        type Settings = DefaultSettings;

        type Error = AssetIoError;

        async fn import(
            _: &mut crate::config::importer::ImportContext<'_>,
            reader: &mut dyn crate::io::AsyncReader,
            _: &crate::asset::AssetMetadata<Self::Settings>,
        ) -> Result<Self::Asset, Self::Error> {
            let mut buf = String::new();
            reader
                .read_to_string(&mut buf)
                .await
                .map_err(AssetIoError::from)
                .map(|_| Text(buf))
        }

        fn extensions() -> &'static [&'static str] {
            &["txt"]
        }
    }

    #[test]
    fn test_load() {
        IoTaskPool::init(TaskPool::builder().build());

        let mut config = AssetConfigBuilder::new();
        config.register::<Text>();
        config.add_importer::<Text>();

        let source = VirtualFs::new();

        smol::block_on(async {
            let mut writer = source.writer("test.txt".as_ref()).await.unwrap();
            writer.write(b"Hello, World!").await.unwrap();
        });

        config.add_source(SourceName::Default, source);
        config.set_cache(AssetCache::new(VirtualFs::new()));

        let db = AssetDatabase::init(config.build());

        let task = db.load::<Text>("test.txt");
        smol::block_on(async move {
            let id = task.await.unwrap();
            let states = db.states.read().await;
            let state = states.get(&id.into()).unwrap();
            assert!(state.state().is_loaded(), "Asset should be loaded");
        });
    }
}
