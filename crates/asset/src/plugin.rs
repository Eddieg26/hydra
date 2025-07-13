use crate::{
    Asset, AssetEvent, AssetId, AssetProcessor, Assets,
    database::{
        AssetDatabase, AssetDatabaseError, LoadDependencies,
        config::{AssetDatabaseBuilder, importer::AssetImporter},
    },
    io::{
        FileSystem,
        local::LocalFs,
        path::{AssetSource, LoadPath},
    },
};
use ecs::app::{End, Plugin};
use serde::Deserialize;

pub struct AssetPlugin;

impl Plugin for AssetPlugin {
    fn name(&self) -> &'static str {
        "AssetPlugin"
    }

    fn setup(&mut self, app: &mut ecs::AppBuilder) {
        app.add_resource(AssetDatabaseBuilder::new());
        app.add_systems(End, AssetDatabase::update_database);
        app.register_event::<AssetDatabaseError>();
    }

    fn finish(&mut self, app: &mut ecs::AppBuilder) {
        let mut builder = app
            .remove_resource::<AssetDatabaseBuilder>()
            .unwrap_or_default();

        if builder
            .config()
            .sources()
            .get(&AssetSource::Default)
            .is_none()
        {
            builder.add_source(AssetSource::Default, LocalFs::new("assets"));
        }

        let commands = std::mem::take(&mut builder.commands);
        let db = AssetDatabase::init(builder);
        db.import();
        smol::block_on(db.send_event(commands));
    }
}

pub trait AssetAppExt {
    fn register_asset<A: Asset>(&mut self) -> &mut Self;
    fn add_loader<A: Asset + for<'a> Deserialize<'a>>(&mut self) -> &mut Self;
    fn add_importer<I: AssetImporter>(&mut self) -> &mut Self;
    fn add_processor<P: AssetProcessor>(&mut self) -> &mut Self;
    fn add_source<S: FileSystem>(
        &mut self,
        name: impl Into<AssetSource<'static>>,
        source: S,
    ) -> &mut Self;
    fn add_asset<A: Asset>(&mut self, id: AssetId<A>, asset: A) -> &mut Self;
    fn load_asset<A: Asset + for<'a> Deserialize<'a>>(
        &mut self,
        path: impl Into<LoadPath<'static>>,
    ) -> &mut Self;
    fn set_default_processor<P: AssetProcessor>(&mut self) -> &mut Self;
    fn set_cache<F: FileSystem>(&mut self, fs: F) -> &mut Self;
}

impl AssetAppExt for ecs::AppBuilder {
    fn register_asset<A: Asset>(&mut self) -> &mut Self {
        self.world_mut().register_asset::<A>();
        self
    }

    fn add_loader<A: Asset + for<'a> Deserialize<'a>>(&mut self) -> &mut Self {
        self.world_mut().add_loader::<A>();
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
        name: impl Into<AssetSource<'static>>,
        source: S,
    ) -> &mut Self {
        self.world_mut().add_source(name, source);
        self
    }

    fn add_asset<A: Asset>(&mut self, id: AssetId<A>, asset: A) -> &mut Self {
        self.world_mut().add_asset(id, asset);
        self
    }

    fn load_asset<A: Asset + for<'a> Deserialize<'a>>(
        &mut self,
        path: impl Into<LoadPath<'static>>,
    ) -> &mut Self {
        self.world_mut().load_asset::<A>(path);
        self
    }

    fn set_default_processor<P: AssetProcessor>(&mut self) -> &mut Self {
        self.world_mut().set_default_processor::<P>();
        self
    }

    fn set_cache<F: FileSystem>(&mut self, fs: F) -> &mut Self {
        self.world_mut().set_cache(fs);
        self
    }
}

impl AssetAppExt for ecs::World {
    fn register_asset<A: Asset>(&mut self) -> &mut Self {
        let config = self.get_or_insert_resource(AssetDatabaseBuilder::new);
        if !config.is_registered::<A>() {
            config.register::<A>();
            self.add_resource(Assets::<A>::new());
            self.register_event::<AssetEvent<A>>();
        }

        self
    }

    fn add_loader<A: Asset + for<'a> Deserialize<'a>>(&mut self) -> &mut Self {
        self.register_asset::<A>();

        let config = self.resource_mut::<AssetDatabaseBuilder>();
        config.add_loader::<A>();

        self
    }

    fn add_importer<I: AssetImporter>(&mut self) -> &mut Self {
        self.register_asset::<I::Asset>()
            .resource_mut::<AssetDatabaseBuilder>()
            .add_importer::<I>();

        self
    }

    fn add_processor<P: AssetProcessor>(&mut self) -> &mut Self {
        self.register_asset::<P::Output>()
            .register_asset::<P::Input>()
            .resource_mut::<AssetDatabaseBuilder>()
            .add_processor::<P>();

        self
    }

    fn add_source<S: FileSystem>(
        &mut self,
        name: impl Into<AssetSource<'static>>,
        source: S,
    ) -> &mut Self {
        let config = self.get_or_insert_resource(AssetDatabaseBuilder::new);
        config.add_source(name, source);
        self
    }

    fn add_asset<A: Asset>(&mut self, id: AssetId<A>, asset: A) -> &mut Self {
        self.register_asset::<A>();

        let mut dependencies = Vec::new();
        asset.get_dependencies(|id| dependencies.push(id));

        let load_deps = if dependencies.is_empty() {
            None
        } else {
            Some(LoadDependencies::new(None, dependencies))
        };

        let builder = self.resource_mut::<AssetDatabaseBuilder>();
        builder.add_asset(id, asset, load_deps);

        self
    }

    fn load_asset<A: Asset + for<'a> Deserialize<'a>>(
        &mut self,
        path: impl Into<LoadPath<'static>>,
    ) -> &mut Self {
        self.register_asset::<A>();

        let builder = self.resource_mut::<AssetDatabaseBuilder>();
        builder.load_asset::<A>(path);

        self
    }

    fn set_default_processor<P: AssetProcessor>(&mut self) -> &mut Self {
        let builder = self.get_or_insert_resource(AssetDatabaseBuilder::new);
        builder.set_default_processor::<P>();

        self
    }

    fn set_cache<F: FileSystem>(&mut self, fs: F) -> &mut Self {
        let builder = self.get_or_insert_resource(AssetDatabaseBuilder::new);
        builder.set_cache(fs);

        self
    }
}
