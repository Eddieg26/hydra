use crate::{
    asset::{Asset, AssetEvent, Assets},
    config::{
        AssetConfigBuilder,
        importer::{AssetImporter, ImportError},
        processor::AssetProcessor,
    },
    database::{AssetDatabase, load::LoadError},
    io::{AssetCache, FileSystem, LocalFs, SourceName},
};
use ecs::app::{End, Plugin};

pub struct AssetPlugin;

impl Plugin for AssetPlugin {
    fn name(&self) -> &'static str {
        "AssetPlugin"
    }

    fn setup(&mut self, app: &mut ecs::AppBuilder) {
        app.add_resource(AssetConfigBuilder::new());
        app.add_systems(End, AssetDatabase::update);
        app.register_event::<ImportError>();
        app.register_event::<LoadError>();
    }

    fn finish(&mut self, app: &mut ecs::AppBuilder) {
        let mut config = app
            .remove_resource::<AssetConfigBuilder>()
            .unwrap_or_default();

        if config.source(&SourceName::Default).is_none() {
            config.add_source(SourceName::Default, LocalFs::new("assets"));
        }

        let database = AssetDatabase::init(config.build());
        database.load_library();
        database.import();
    }
}

pub trait AssetAppExt {
    fn register_asset<A: Asset>(&mut self) -> &mut Self;
    fn add_importer<I: AssetImporter>(&mut self) -> &mut Self;
    fn add_processor<P: AssetProcessor>(&mut self) -> &mut Self;
    fn add_source<S: FileSystem>(
        &mut self,
        name: impl Into<SourceName<'static>>,
        source: S,
    ) -> &mut Self;
    fn set_cache(&mut self, cache: AssetCache) -> &mut Self;
}

impl AssetAppExt for ecs::AppBuilder {
    fn register_asset<A: Asset>(&mut self) -> &mut Self {
        self.world_mut().register_asset::<A>();
        self
    }

    fn add_importer<I: AssetImporter>(&mut self) -> &mut Self {
        self.world_mut().add_importer::<I>();
        self
    }

    fn add_processor<P: AssetProcessor>(&mut self) -> &mut Self {
        self.world_mut().add_processor::<P>();
        self
    }

    fn add_source<S: FileSystem>(
        &mut self,
        name: impl Into<SourceName<'static>>,
        source: S,
    ) -> &mut Self {
        self.world_mut().add_source(name, source);
        self
    }

    fn set_cache(&mut self, cache: AssetCache) -> &mut Self {
        self.world_mut().set_cache(cache);
        self
    }
}

impl AssetAppExt for ecs::World {
    fn register_asset<A: Asset>(&mut self) -> &mut Self {
        let config = self.get_or_insert_resource(AssetConfigBuilder::new);
        if !config.is_registered::<A>() {
            config.register::<A>();
            self.add_resource(Assets::<A>::new());
            self.register_event::<AssetEvent<A>>();
        }

        self
    }

    fn add_importer<I: AssetImporter>(&mut self) -> &mut Self {
        self.register_asset::<I::Asset>()
            .resource_mut::<AssetConfigBuilder>()
            .add_importer::<I>();

        self
    }

    fn add_processor<P: AssetProcessor>(&mut self) -> &mut Self {
        self.register_asset::<P::Output>()
            .register_asset::<<P::Input as AssetImporter>::Asset>()
            .resource_mut::<AssetConfigBuilder>()
            .add_processor::<P>();

        self
    }

    fn add_source<S: FileSystem>(
        &mut self,
        name: impl Into<SourceName<'static>>,
        source: S,
    ) -> &mut Self {
        let config = self.get_or_insert_resource(AssetConfigBuilder::new);
        config.add_source(name, source);
        self
    }

    fn set_cache(&mut self, cache: AssetCache) -> &mut Self {
        let config = self.get_or_insert_resource(AssetConfigBuilder::new);
        config.set_cache(cache);
        self
    }
}
