use crate::{
    config::{AssetConfig, importer::ImportError},
    io::cache::AssetLibrary,
};
use ecs::EventWriter;
use smol::{
    channel::{Receiver, Sender, unbounded},
    lock::RwLock,
};
use state::AssetStates;
use std::sync::{Arc, OnceLock};

pub mod events;
pub mod import;
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

    pub fn update(&self, mut events: EventWriter<ImportError>) {
        while let Ok(event) = self.receiver.try_recv() {
            match event {
                DatabaseEvent::ImportError(error) => events.send(error),
            }
        }
    }
}

pub enum DatabaseEvent {
    ImportError(ImportError),
}

impl From<ImportError> for DatabaseEvent {
    fn from(value: ImportError) -> Self {
        DatabaseEvent::ImportError(value)
    }
}
