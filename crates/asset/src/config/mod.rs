use crate::{
    AssetCommand, AssetId,
    asset::{Asset, AssetType},
    database::{commands::LoadDependencies, load::LoadPath},
    io::{
        AssetSource, FileSystem,
        cache::AssetCache,
        local::LocalFs,
        source::{AssetSources, SourceName},
    },
};
use ecs::Resource;
use importer::{AssetImporter, AssetImporters};
use processor::{AssetProcessor, AssetProcessors};
use registry::AssetRegistry;
use serde::Deserialize;
use std::{any::TypeId, collections::HashSet};

pub mod importer;
pub mod processor;
pub mod registry;

pub type BoxedError = Box<dyn std::error::Error + Send + Sync + 'static>;

#[derive(Resource)]
pub struct AssetConfigBuilder {
    registry: AssetRegistry,
    importers: AssetImporters,
    processors: AssetProcessors,
    sources: AssetSources,
    cache: AssetCache,
    pub(crate) load: HashSet<LoadPath<'static>>,
    pub(crate) commands: Vec<AssetCommand>,
}

impl AssetConfigBuilder {
    pub fn new() -> Self {
        Self {
            registry: AssetRegistry::new(),
            importers: AssetImporters::new(),
            processors: AssetProcessors::new(),
            sources: AssetSources::new(),
            cache: AssetCache::new(LocalFs::new(".cache")),
            load: HashSet::new(),
            commands: Vec::new(),
        }
    }

    pub fn register<A: Asset>(&mut self) -> AssetType {
        self.registry.register::<A>()
    }

    pub fn add_importer<I: AssetImporter>(&mut self) {
        self.importers.add::<I>();
        let ty = self.register::<I::Asset>();
        self.registry.set_deserialize::<I::Asset>(ty);
    }

    pub fn add_processor<P: AssetProcessor>(&mut self) {
        self.processors.add::<P>();
        self.register::<P::Output>();
        self.register::<<P::Input as AssetImporter>::Asset>();
    }

    pub fn add_source<F: FileSystem>(&mut self, name: impl Into<SourceName<'static>>, fs: F) {
        self.sources.add(name.into(), fs);
    }

    pub fn set_cache(&mut self, cache: AssetCache) {
        self.cache = cache
    }

    pub fn is_registered<A: Asset>(&self) -> bool {
        self.registry.contains(TypeId::of::<A>())
    }

    pub fn source(&self, name: &SourceName<'static>) -> Option<&AssetSource> {
        self.sources.get(name)
    }

    pub fn add_asset<A: Asset>(
        &mut self,
        id: AssetId<A>,
        asset: A,
        dependencies: Option<LoadDependencies>,
    ) {
        let ty = self.register::<A>();
        self.commands.push(AssetCommand::Add {
            id: id.into(),
            ty,
            asset: asset.into(),
            dependencies,
        });
    }

    pub fn load_asset<A: Asset + for<'a> Deserialize<'a>>(
        &mut self,
        path: impl Into<LoadPath<'static>>,
    ) {
        self.load.insert(path.into());
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

impl Default for AssetConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

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

    pub fn is_registered(&self, ty: AssetType) -> bool {
        self.registry.get(ty).is_some()
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
