use crate::{
    Component, ComponentId, Components, Entities, Event, EventId, EventRegistry, ModeId, Phase,
    Resource, ResourceId, Resources, RunMode, Schedule, Systems, World, WorldMode, ext,
    world::{Archetypes, WorldCell},
};
use std::{
    collections::{HashMap, HashSet},
    hash::Hash,
    thread::JoinHandle,
};

#[allow(unused_variables)]
pub trait Plugin: 'static {
    fn name(&self) -> &'static str {
        ext::short_type_name::<Self>()
    }

    /// Setup is called when the plugin is added to the app.
    /// It is used to register systems, resources, and other app components.
    fn setup(&mut self, app: &mut AppBuilder);

    /// Run is called when the app is being built.
    fn run(&mut self, app: &mut AppBuilder) {}

    /// Finish is called after all of a plugin's dependencies have been added and ran.
    fn finish(&mut self, app: &mut AppBuilder) {}
}

pub trait Plugins {
    fn add_plugin<P: Plugin>(&mut self, plugin: P) -> &mut Self;
}

pub trait PluginKit {
    fn get<P: Plugins>(self, plugins: &mut P);
}

impl<T: Plugin> PluginKit for T {
    fn get<P: Plugins>(self, plugins: &mut P) {
        plugins.add_plugin(self);
    }
}

pub trait AppTag: 'static {
    fn name(&self) -> &'static str {
        ext::short_type_name::<Self>()
    }
}

impl Hash for Box<dyn AppTag> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.name().hash(state);
    }
}

impl PartialEq for Box<dyn AppTag> {
    fn eq(&self, other: &Self) -> bool {
        self.name() == other.name()
    }
}

impl Eq for Box<dyn AppTag> {}

pub struct Init;
impl Phase for Init {}

pub struct Run;
impl Phase for Run {}

pub struct Extract;
impl Phase for Extract {}

pub struct Shutdown;
impl Phase for Shutdown {}

pub struct AppBuildInfo {
    world: World,
    schedule: Schedule,
    plugins: Vec<Box<dyn Plugin>>,
    registered: HashSet<&'static str>,
}

impl AppBuildInfo {
    pub fn new() -> Self {
        Self {
            world: World::new(),
            schedule: Schedule::new(RunMode::Sequential),
            plugins: Vec::new(),
            registered: HashSet::new(),
        }
    }
}

pub enum AppType {
    Main {
        config: AppBuildInfo,
        secondary: HashMap<Box<dyn AppTag>, AppBuilder>,
        runner: Option<Box<dyn Fn(Apps) -> Apps>>,
    },
    Sub(AppBuildInfo),
}

pub struct AppBuilder(AppType);
impl AppBuilder {
    pub fn new() -> Self {
        Self(AppType::Main {
            config: AppBuildInfo::new(),
            secondary: HashMap::new(),
            runner: None,
        })
    }

    pub fn single() -> Self {
        Self(AppType::Sub(AppBuildInfo::new()))
    }

    pub fn components(&self) -> &Components {
        self.world().components()
    }

    pub fn archetypes(&self) -> &Archetypes {
        self.world().archetypes()
    }

    pub fn resources(&self) -> &Resources {
        self.world().resources()
    }

    pub fn entities(&self) -> &Entities {
        self.world().entities()
    }

    pub fn events(&self) -> &EventRegistry {
        self.world().events()
    }

    pub fn register<C: Component>(&mut self) -> ComponentId {
        self.world_mut().register::<C>()
    }

    pub fn register_resource<R: Resource + Send>(&mut self) -> ResourceId {
        self.world_mut().register_resource::<R>()
    }

    pub fn register_non_send_resource<R: Resource>(&mut self) -> ResourceId {
        self.world_mut().register_non_send_resource::<R>()
    }

    pub fn register_event<E: Event>(&mut self) -> EventId {
        self.world_mut().register_event::<E>()
    }

    pub fn add_mode<M: WorldMode>(&mut self) -> ModeId {
        self.world_mut().add_mode::<M>()
    }

    pub fn add_resource<R: Resource + Send>(&mut self, resource: R) -> ResourceId {
        self.world_mut().add_resource(resource)
    }

    pub fn add_non_send_resource<R: Resource>(&mut self, resource: R) -> ResourceId {
        self.world_mut().add_non_send_resource(resource)
    }

    pub fn resource<R: Resource + Send>(&self) -> &R {
        self.world().resource::<R>()
    }

    pub fn resource_mut<R: Resource + Send>(&mut self) -> &mut R {
        self.world_mut().resource_mut::<R>()
    }

    pub fn try_resource<R: Resource + Send>(&self) -> Option<&R> {
        self.world().try_resource::<R>()
    }

    pub fn try_resource_mut<R: Resource + Send>(&mut self) -> Option<&mut R> {
        self.world_mut().try_resource_mut::<R>()
    }

    pub fn non_send_resource<R: Resource>(&self) -> &R {
        self.world().non_send_resource::<R>()
    }

    pub fn non_send_resource_mut<R: Resource>(&mut self) -> &mut R {
        self.world_mut().non_send_resource_mut::<R>()
    }

    pub fn try_non_send_resource<R: Resource>(&self) -> Option<&R> {
        self.world().try_non_send_resource::<R>()
    }

    pub fn try_non_send_resource_mut<R: Resource>(&mut self) -> Option<&mut R> {
        self.world_mut().try_non_send_resource_mut::<R>()
    }

    pub fn get_or_insert_resource<R: Resource + Send>(&mut self, get: impl Fn() -> R) -> &mut R {
        self.world_mut().get_or_insert_resource::<R>(get)
    }

    pub fn get_or_insert_non_send_resource<R: Resource>(&mut self, get: impl Fn() -> R) -> &mut R {
        self.world_mut().get_or_insert_non_send_resource::<R>(get)
    }

    pub fn remove_resource<R: Resource>(&mut self) -> Option<R> {
        self.world_mut().remove_resource::<R>()
    }

    pub fn add_sub_app(&mut self, app: impl AppTag) {
        let app = Box::new(app) as Box<dyn AppTag>;
        match &mut self.0 {
            AppType::Main { secondary, .. } => {
                secondary.entry(app).or_insert(AppBuilder::single());
            }
            AppType::Sub(_) => panic!("Cannot add sub app to a sub app"),
        }
    }

    pub fn sub_app(&self, app: impl AppTag) -> Option<&AppBuilder> {
        match &self.0 {
            AppType::Main { secondary, .. } => secondary.get(&(Box::new(app) as Box<dyn AppTag>)),
            AppType::Sub(_) => None,
        }
    }

    pub fn sub_app_mut(&mut self, app: impl AppTag) -> Option<&mut AppBuilder> {
        match &mut self.0 {
            AppType::Main { secondary, .. } => {
                secondary.get_mut(&(Box::new(app) as Box<dyn AppTag>))
            }
            AppType::Sub(_) => None,
        }
    }

    pub fn add_plugins<P: PluginKit>(&mut self, plugins: P) {
        plugins.get(self);
    }

    pub fn build(self) -> Apps {
        match self.0 {
            AppType::Main {
                config,
                secondary,
                runner,
            } => {
                let mut main = Self(AppType::Main {
                    config,
                    secondary,
                    runner,
                });

                main.build_plugins();

                let AppType::Main {
                    config,
                    secondary,
                    runner,
                } = main.0
                else {
                    panic!("Expected AppConfigKind::Main");
                };

                let main = App::from(config);
                let sub = secondary
                    .into_values()
                    .map(|mut app| {
                        app.build_plugins();

                        App::from(app.into_build_info())
                    })
                    .collect::<Vec<_>>();

                match runner {
                    Some(runner) => runner(Apps::new(main, sub)),
                    None => Self::default_runner(Apps::new(main, sub)),
                }
            }
            AppType::Sub(config) => {
                let mut sub = Self(AppType::Sub(config));
                sub.build_plugins();

                let app = App::from(sub.into_build_info());
                Apps::new(app, vec![])
            }
        }
    }

    fn build_plugins(&mut self) {
        let mut plugins = std::mem::take(self.plugins_mut());
        let mut queue: Vec<Box<dyn Plugin>> = vec![];
        while !plugins.is_empty() {
            for plugin in &mut plugins {
                plugin.run(self);
            }

            if self.plugins().is_empty() {
                for mut plugin in plugins.drain(..) {
                    plugin.finish(self);
                }

                if self.plugins().is_empty() {
                    for mut plugin in queue.drain(..) {
                        plugin.finish(self);
                    }
                }
            } else {
                queue.append(&mut plugins);
            }

            plugins.append(self.plugins_mut());
        }
    }

    fn info(&self) -> &AppBuildInfo {
        match &self.0 {
            AppType::Main { config, .. } => config,
            AppType::Sub(config) => config,
        }
    }

    fn info_mut(&mut self) -> &mut AppBuildInfo {
        match &mut self.0 {
            AppType::Main { config, .. } => config,
            AppType::Sub(config) => config,
        }
    }

    fn plugins(&self) -> &Vec<Box<dyn Plugin>> {
        &self.info().plugins
    }

    fn plugins_mut(&mut self) -> &mut Vec<Box<dyn Plugin>> {
        &mut self.info_mut().plugins
    }

    fn into_build_info(self) -> AppBuildInfo {
        match self.0 {
            AppType::Main { config, .. } => config,
            AppType::Sub(config) => config,
        }
    }

    fn world(&self) -> &World {
        &self.info().world
    }

    fn world_mut(&mut self) -> &mut World {
        &mut self.info_mut().world
    }

    fn default_runner(mut apps: Apps) -> Apps {
        apps.init();
        apps.run();
        apps.shutdown();
        apps.await_apps();

        apps
    }
}

impl Plugins for AppBuilder {
    fn add_plugin<P: Plugin>(&mut self, mut plugin: P) -> &mut Self {
        if !self.info().registered.contains(plugin.name()) {
            self.info_mut().registered.insert(plugin.name());

            plugin.setup(self);
            self.plugins_mut().push(Box::new(plugin));
        }

        self
    }
}

impl From<AppBuildInfo> for App {
    fn from(mut value: AppBuildInfo) -> Self {
        let systems = value.schedule.build(&mut value.world).unwrap();
        App {
            world: value.world,
            systems,
        }
    }
}

pub struct App {
    world: World,
    systems: Systems,
}

impl App {
    pub fn new() -> AppBuilder {
        AppBuilder::new()
    }

    pub fn single() -> AppBuilder {
        AppBuilder::single()
    }

    pub fn world(&self) -> &World {
        &self.world
    }

    pub fn systems(&self) -> &Systems {
        &self.systems
    }

    pub fn is_send(&self) -> bool {
        self.world.resources().is_send()
    }

    pub fn run(&mut self, phase: impl Phase) {
        self.systems.run(phase, &mut self.world);
        self.world.update();
    }

    fn extract(&mut self, main: MainWorld) {
        self.world.add_resource(main);
        self.run(Extract);
        self.world.remove_resource::<MainWorld>();
    }

    fn run_once(mut self, phase: impl Phase) -> Self {
        self.systems.run(phase, &mut self.world);
        self.world.update();
        self
    }
}

pub struct Apps {
    main: App,
    sub: Vec<App>,
    handles: Vec<JoinHandle<App>>,
}

impl Apps {
    fn new(main: App, sub: Vec<App>) -> Self {
        println!("SUB: {}", sub.len());
        Self {
            main,
            sub,
            handles: Vec::new(),
        }
    }

    pub fn world(&self) -> &World {
        &self.main.world
    }

    pub fn init(&mut self) {
        self.main.run(Init);
    }

    pub fn run(&mut self) {
        self.await_apps();

        self.main.run(Run);

        let mut handles = Vec::new();
        let main = MainWorld::new(&mut self.main.world);
        self.sub = self
            .sub
            .drain(..)
            .filter_map(|mut app| {
                app.extract(main);

                if app.is_send() {
                    handles.push(std::thread::spawn(move || app.run_once(Run)));
                    None
                } else {
                    Some(app)
                }
            })
            .collect();

        for app in &mut self.sub {
            app.run(Run);
        }

        self.handles = handles;
    }

    pub fn shutdown(&mut self) {
        self.main.run(Shutdown);
    }

    fn await_apps(&mut self) {
        let apps = self.handles.drain(..).map(|handle| handle.join().unwrap());
        self.sub.extend(apps);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MainWorld(WorldCell<'static>);
impl MainWorld {
    pub(crate) fn new(world: &mut World) -> Self {
        let cell: WorldCell<'static> = unsafe { std::mem::transmute(world.cell()) };
        Self(cell)
    }
}

impl std::ops::Deref for MainWorld {
    type Target = World;

    fn deref(&self) -> &Self::Target {
        unsafe { self.0.get() }
    }
}

impl std::ops::DerefMut for MainWorld {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { self.0.get_mut() }
    }
}

impl Resource for MainWorld {}
unsafe impl Send for MainWorld {}
unsafe impl Sync for MainWorld {}

#[allow(unused_imports, dead_code)]
mod tests {
    use super::{App, AppTag, Plugin, Plugins};
    use crate::Resource;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Value(usize);
    impl Resource for Value {}

    pub struct PluginA;
    impl Plugin for PluginA {
        fn setup(&mut self, app: &mut super::AppBuilder) {
            assert_eq!(app.resource::<Value>().0, 0);
            app.resource_mut::<Value>().0 = 1;
            app.add_plugins(PluginB);
        }
    }

    pub struct PluginB;
    impl Plugin for PluginB {
        fn setup(&mut self, _: &mut super::AppBuilder) {}

        fn run(&mut self, app: &mut super::AppBuilder) {
            assert_eq!(app.resource::<Value>().0, 1);
            app.resource_mut::<Value>().0 = 2;
            app.add_plugins(PluginC);
        }
    }

    pub struct PluginC;
    impl Plugin for PluginC {
        fn setup(&mut self, _: &mut super::AppBuilder) {}

        fn finish(&mut self, app: &mut super::AppBuilder) {
            assert_eq!(app.resource::<Value>().0, 2);
            app.resource_mut::<Value>().0 = 3;
            app.add_plugins(PluginD);
        }
    }

    pub struct PluginD;
    impl Plugin for PluginD {
        fn setup(&mut self, app: &mut super::AppBuilder) {
            assert_eq!(app.resource::<Value>().0, 3);
            app.resource_mut::<Value>().0 = 4;
        }
    }

    pub struct TestApp;
    impl AppTag for TestApp {}

    #[test]
    fn build_plugins() {
        let mut builder = App::single();
        builder.add_resource(Value(0));
        builder.add_plugin(PluginA);

        builder.build_plugins();

        assert_eq!(builder.resource::<Value>().0, 4);
    }

    #[test]
    fn build_multi_app() {
        let mut builder = App::new();
        builder.add_resource(Value(0));
        builder.add_plugins(PluginA);
        builder.add_sub_app(TestApp);
        builder.sub_app_mut(TestApp).unwrap().add_resource(Value(0));
        builder.sub_app_mut(TestApp).unwrap().add_plugins(PluginA);

        let app = builder.build();

        assert_eq!(app.world().resource::<Value>().0, 4);
        assert_eq!(app.sub[0].world().resource::<Value>().0, 4);
    }
}
