use crate::{
    AssetId,
    asset::{Asset, AssetType},
    database::{AddAsset, LoadAsset, state::LoadDependencies},
    ext::DeserializeExt,
    io::{
        FileSystem,
        local::LocalFs,
        path::{AssetSource, LoadPath},
    },
};
use ecs::{CommandBuffer, Resource};

pub mod cache;
pub mod importer;
pub mod registry;
pub mod sources;

pub use cache::*;
pub use importer::*;
pub use registry::*;
pub use sources::*;

pub struct AssetDatabaseConfig {
    registry: AssetRegistry,
    importers: AssetImporters,
    processors: AssetProcessors,
    sources: AssetSources,
    cache: AssetCache,
}

impl AssetDatabaseConfig {
    fn new() -> Self {
        Self {
            registry: AssetRegistry::new(),
            importers: AssetImporters::new(),
            processors: AssetProcessors::new(),
            sources: AssetSources::new(),
            cache: AssetCache::new(LocalFs::new(".cache")),
        }
    }

    pub fn registry(&self) -> &AssetRegistry {
        &self.registry
    }

    pub fn importers(&self) -> &AssetImporters {
        &self.importers
    }

    pub fn processors(&self) -> &AssetProcessors {
        &self.processors
    }

    pub fn sources(&self) -> &AssetSources {
        &self.sources
    }

    pub fn cache(&self) -> &AssetCache {
        &self.cache
    }
}

#[derive(Resource)]
pub struct AssetDatabaseBuilder {
    pub(super) config: AssetDatabaseConfig,
    pub(crate) commands: CommandBuffer,
}

impl AssetDatabaseBuilder {
    pub fn new() -> Self {
        Self {
            config: AssetDatabaseConfig::new(),
            commands: CommandBuffer::new(),
        }
    }

    pub fn config(&self) -> &AssetDatabaseConfig {
        &self.config
    }

    pub fn is_registered<A: Asset>(&self) -> bool {
        self.config.registry().is_registered::<A>()
    }

    pub fn register<A: Asset>(&mut self) -> AssetType {
        self.config.registry.register::<A>()
    }

    pub fn add_importer<I: AssetImporter>(&mut self) {
        self.config.registry.set_deserialize::<I::Asset>();
        self.config.importers.add::<I>()
    }

    pub fn add_loader<A: Asset + DeserializeExt>(&mut self) {
        self.config.registry.set_deserialize::<A>();
    }

    pub fn add_processor<P: AssetProcessor>(&mut self) -> AssetProcessorId {
        self.config.registry.set_deserialize::<P::Input>();
        self.config.registry.set_deserialize::<P::Output>();
        self.config.processors.add::<P>()
    }

    pub fn add_source<F: FileSystem>(&mut self, source: impl Into<AssetSource<'static>>, fs: F) {
        self.config.sources.add(source, fs);
    }

    pub fn add_asset<A: Asset>(
        &mut self,
        id: AssetId<A>,
        asset: A,
        dependencies: Option<LoadDependencies>,
    ) {
        self.commands.add(AddAsset::new(id, asset, dependencies));
    }

    pub fn load_asset<A: Asset + DeserializeExt>(&mut self, path: impl Into<LoadPath<'static>>) {
        self.commands.add(LoadAsset::<A>::from(path.into()));
    }

    pub fn set_cache<F: FileSystem>(&mut self, fs: F) {
        self.config.cache = AssetCache::new(fs);
    }

    pub fn set_default_processor<P: AssetProcessor>(&mut self) {
        self.add_processor::<P>();
        let ty = self.register::<P::Input>();
        self.config.processors.set_default::<P>(ty);
    }
}

impl Default for AssetDatabaseBuilder {
    fn default() -> Self {
        Self::new()
    }
}
