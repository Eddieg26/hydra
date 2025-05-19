use crate::{config::AssetConfig, io::cache::AssetLibrary};
use smol::lock::RwLock;
use std::sync::Arc;

pub mod events;
pub mod state;

pub struct AssetDatabase {
    config: Arc<AssetConfig>,
    library: Arc<RwLock<AssetLibrary>>,
}
