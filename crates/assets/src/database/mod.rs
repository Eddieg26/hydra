use crate::{
    asset::{ErasedAsset, ErasedId},
    config::{AssetConfig, importer::ImportError},
    io::{ArtifactMeta, AssetPath, cache::AssetLibrary},
};
use ecs::EventWriter;
use load::AssetLoadError;
use smol::{
    channel::{Receiver, Sender, unbounded},
    lock::RwLock,
};
use state::AssetStates;
use std::sync::{Arc, OnceLock};

pub mod events;
pub mod import;
pub mod load;
pub mod state;

static DB: OnceLock<AssetDatabase> = OnceLock::new();

#[derive(Debug, Clone)]
pub struct AssetDatabase {
    config: Arc<AssetConfig>,
    library: Arc<RwLock<AssetLibrary>>,
    states: Arc<RwLock<AssetStates>>,
    writer: Arc<RwLock<()>>,
    sender: Sender<DatabaseEvent>,
    receiver: Receiver<DatabaseEvent>,
}

impl AssetDatabase {
    pub fn init(config: AssetConfig) {
        DB.set(AssetDatabase::new(config))
            .expect("AssetDatabase already initialized");
    }

    pub fn get() -> &'static AssetDatabase {
        DB.get().expect("AssetDatabase not initialized")
    }

    fn new(config: AssetConfig) -> Self {
        let (sender, receiver) = unbounded();

        Self {
            config: Arc::new(config),
            library: Arc::default(),
            states: Arc::default(),
            writer: Arc::default(),
            sender,
            receiver,
        }
    }

    pub fn config(&self) -> &AssetConfig {
        &self.config
    }

    pub fn get_id<'a>(&self, path: impl Into<AssetPath<'a>>) -> Option<ErasedId> {
        self.library
            .read_blocking()
            .get_id(&path.into())
            .map(|id| *id)
    }

    pub fn get_path(&self, id: ErasedId) -> Option<AssetPath<'static>> {
        self.library.read_blocking().get_path(id).cloned()
    }

    pub fn update(&self, mut events: EventWriter<ImportError>) {
        while let Ok(event) = self.receiver.try_recv() {
            match event {
                DatabaseEvent::ImportError(error) => events.send(error),
                DatabaseEvent::AssetLoaded { asset, meta } => {}
                DatabaseEvent::LoadError(error) => {}
            }
        }
    }
}

pub enum DatabaseEvent {
    AssetLoaded {
        asset: ErasedAsset,
        meta: ArtifactMeta,
    },
    ImportError(ImportError),
    LoadError(AssetLoadError),
}

impl From<ImportError> for DatabaseEvent {
    fn from(value: ImportError) -> Self {
        DatabaseEvent::ImportError(value)
    }
}
