use crate::{
    asset::{Asset, AssetType},
    io::{
        FileSystem,
        cache::AssetCache,
        local::LocalFs,
        source::{AssetSources, SourceName},
    },
};
use ecs::Resource;
use importer::{AssetImporter, AssetImporters};
use processor::{AssetProcessor, AssetProcessors};
use registry::AssetRegistry;

pub mod importer;
pub mod processor;
pub mod registry;

pub type BoxedError = Box<dyn std::error::Error + Send + Sync + 'static>;

pub struct AssetConfigBuilder {
    registry: AssetRegistry,
    importers: AssetImporters,
    processors: AssetProcessors,
    sources: AssetSources,
    cache: AssetCache,
}

impl AssetConfigBuilder {
    pub fn new() -> Self {
        Self {
            registry: AssetRegistry::new(),
            importers: AssetImporters::new(),
            processors: AssetProcessors::new(),
            sources: AssetSources::new(),
            cache: AssetCache::new(LocalFs::new(".cache")),
        }
    }

    pub fn register<A: Asset>(&mut self) -> AssetType {
        self.registry.register::<A>()
    }

    pub fn add_importer<I: AssetImporter>(&mut self) {
        self.importers.add::<I>();
        self.register::<I::Asset>();
    }

    pub fn add_processor<P: AssetProcessor>(&mut self) {
        self.processors.add::<P>();
        self.register::<P::Output>();
        self.register::<<P::Input as AssetImporter>::Asset>();
    }

    pub fn add_source<F: FileSystem>(&mut self, name: impl Into<SourceName<'static>>, fs: F) {
        self.sources.add(name.into(), fs);
    }

    pub fn set_cache<F: FileSystem>(&mut self, fs: F) {
        self.cache = AssetCache::new(fs);
    }

    pub fn build(self) -> AssetConfig {
        AssetConfig {
            registry: self.registry,
            importers: self.importers,
            processors: self.processors,
            sources: self.sources,
            cache: self.cache,
        }
    }
}

impl Resource for AssetConfigBuilder {}

#[derive(Debug)]
pub struct AssetConfig {
    registry: AssetRegistry,
    importers: AssetImporters,
    processors: AssetProcessors,
    sources: AssetSources,
    cache: AssetCache,
}

impl AssetConfig {
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
