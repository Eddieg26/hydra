use crate::{
    asset::{Asset, Assets},
    config::{AssetConfigBuilder, importer::AssetImporter, processor::AssetProcessor},
    database::{AssetDatabase, load::LoadPath},
    io::{FileSystem, LocalFs, SourceName},
};
use ecs::app::{End, Plugin};

pub struct AssetPlugin;

impl Plugin for AssetPlugin {
    fn setup(&mut self, app: &mut ecs::AppBuilder) {
        app.add_resource(AssetConfigBuilder::new());
        app.add_systems(End, AssetDatabase::update);
    }

    fn finish(&mut self, app: &mut ecs::AppBuilder) {
        let mut config = app
            .remove_resource::<AssetConfigBuilder>()
            .unwrap_or_default();

        if config.source(&SourceName::Default).is_none() {
            config.add_source(SourceName::Default, LocalFs::new("assets"));
        }

        let load_paths = std::mem::take(config.load_paths_mut());
        let database = AssetDatabase::init(config.build());
        database.load_library();
        database.import();
        database.load_paths(load_paths);
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
    fn set_cache<S: FileSystem>(&mut self, fs: S) -> &mut Self;
    fn load_asset(&mut self, path: impl Into<LoadPath<'static>>) -> &mut Self;
}

impl AssetAppExt for ecs::AppBuilder {
    fn register_asset<A: Asset>(&mut self) -> &mut Self {
        let config = self.get_or_insert_resource(AssetConfigBuilder::new);
        if !config.is_registered::<A>() {
            config.register::<A>();
            self.add_resource(Assets::<A>::new());
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

    fn set_cache<S: FileSystem>(&mut self, fs: S) -> &mut Self {
        let config = self.get_or_insert_resource(AssetConfigBuilder::new);
        config.set_cache(fs);
        self
    }

    fn load_asset(&mut self, path: impl Into<LoadPath<'static>>) -> &mut Self {
        let config = self.get_or_insert_resource(AssetConfigBuilder::new);
        config.add_load_path(path.into());
        self
    }
}
