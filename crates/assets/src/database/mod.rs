use crate::{config::AssetConfig, io::cache::AssetLibrary};
use ecs::Resource;
use events::{DatabaseCommand, DatabaseCommands, DatabaseEvent};
use smol::{
    channel::{Receiver, Sender, unbounded},
    lock::RwLock,
};
use state::AssetStates;
use std::sync::Arc;

pub mod events;
pub mod state;

#[derive(Clone)]
pub struct AssetDatabase {
    config: Arc<AssetConfig>,
    library: Arc<RwLock<AssetLibrary>>,
    states: Arc<RwLock<AssetStates>>,
    commands: DatabaseCommands,
    sender: Sender<DatabaseEvent>,
    receiver: Receiver<DatabaseEvent>,
}

impl AssetDatabase {
    pub fn new(config: AssetConfig) -> Self {
        let (sender, receiver) = unbounded();

        Self {
            config: Arc::new(config),
            library: Arc::default(),
            states: Arc::default(),
            commands: DatabaseCommands::default(),
            sender,
            receiver,
        }
    }

    pub fn config(&self) -> &AssetConfig {
        &self.config
    }

    pub fn execute<C: DatabaseCommand>(&self, command: C) {
        self.commands.execute(command, self.clone());
    }

    pub fn update(&self) {
        while let Ok(_event) = self.receiver.try_recv() {}
    }
}

impl Resource for AssetDatabase {}
