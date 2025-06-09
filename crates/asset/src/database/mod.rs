use crate::{
    asset::ErasedId,
    config::{AssetConfig, AssetConfigBuilder, importer::ImportError},
    database::load::LoadPath,
    io::{AssetPath, cache::AssetLibrary},
};
use commands::AssetCommand;
use ecs::{Commands, EventWriter, SystemArg, core::task::IoTaskPool};
use load::LoadError;
use smol::{
    channel::{Receiver, Sender, unbounded},
    lock::RwLock,
};
use state::AssetStates;
use std::sync::{Arc, OnceLock};

pub mod commands;
pub mod import;
pub mod load;
pub mod state;

static DB: OnceLock<AssetDatabase> = OnceLock::new();

#[derive(Debug)]
pub struct AssetDatabase {
    config: Arc<AssetConfig>,
    library: RwLock<AssetLibrary>,
    states: RwLock<AssetStates>,
    writer: RwLock<Option<()>>,
    sender: Sender<DatabaseEvent>,
    receiver: Receiver<DatabaseEvent>,
}

impl AssetDatabase {
    pub fn init(config: AssetConfig) -> &'static AssetDatabase {
        DB.get_or_init(|| Self::new(config))
    }

    pub fn get() -> &'static AssetDatabase {
        DB.get()
            .expect("AssetDatabase not initialized. Make sure to add AssetPlugin to your app.")
    }

    pub fn is_initialized() -> bool {
        DB.get().is_some()
    }

    fn new(config: AssetConfig) -> Self {
        let (sender, receiver) = unbounded();

        Self {
            config: Arc::new(config),
            library: RwLock::default(),
            states: RwLock::default(),
            writer: RwLock::new(Some(())),
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

    pub async fn send_event(&self, event: impl Into<DatabaseEvent>) {
        let _ = self.sender.send(event.into()).await;
    }

    fn setup(&self) {
        IoTaskPool::get()
            .spawn(async {
                let db = AssetDatabase::get();

                let mut writer = db.writer.write().await;
                *writer = Some(());

                for (_, source) in db.config.sources().iter() {
                    if !source.exists("".as_ref()).await.unwrap_or(false) {
                        let _ = source.create_dir_all("".as_ref()).await;
                    }
                }

                let _ = db.config.cache().create_root().await;

                let library = match db.config.cache().load_library().await {
                    Ok(lib) => lib,
                    Err(_) => AssetLibrary::new(),
                };

                db.library.write().await.replace(library);
            })
            .detach();
    }

    pub fn update(
        db: &AssetDatabase,
        mut import_errors: EventWriter<ImportError>,
        mut load_errors: EventWriter<LoadError>,
        mut commands: Commands,
    ) {
        while let Ok(event) = db.receiver.try_recv() {
            match event {
                DatabaseEvent::Setup => db.setup(),
                DatabaseEvent::LoadAsset(path) => {
                    let _ = db.load_erased(path);
                }
                DatabaseEvent::AssetCommand(command) => commands.add(command),
                DatabaseEvent::ImportError(error) => import_errors.send(error),
                DatabaseEvent::LoadError(error) => load_errors.send(error),
            }
        }
    }
}

pub enum DatabaseEvent {
    Setup,
    LoadAsset(LoadPath<'static>),
    AssetCommand(AssetCommand),
    ImportError(ImportError),
    LoadError(LoadError),
}

impl From<AssetCommand> for DatabaseEvent {
    fn from(value: AssetCommand) -> Self {
        Self::AssetCommand(value)
    }
}

impl From<ImportError> for DatabaseEvent {
    fn from(value: ImportError) -> Self {
        Self::ImportError(value)
    }
}

impl From<LoadError> for DatabaseEvent {
    fn from(value: LoadError) -> Self {
        Self::LoadError(value)
    }
}

unsafe impl SystemArg for &AssetDatabase {
    type Item<'world, 'state> = &'static AssetDatabase;

    type State = &'static AssetDatabase;

    fn init(world: &mut ecs::World, _: &mut ecs::WorldAccess) -> Self::State {
        if AssetDatabase::is_initialized() {
            AssetDatabase::get()
        } else if let Some(config) = world.remove_resource::<AssetConfigBuilder>() {
            AssetDatabase::init(config.build())
        } else {
            panic!("AssetDatabase not initialized and no AssetConfigBuilder found in world");
        }
    }

    unsafe fn get<'world, 'state>(
        state: &'state mut Self::State,
        _: ecs::world::WorldCell<'world>,
        _: &ecs::SystemMeta,
    ) -> Self::Item<'world, 'state> {
        state
    }
}
