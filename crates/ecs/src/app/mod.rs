use crate::{
    Component, Components, Entities, Event, EventRegistry, IntoSystemConfigs, Phase, Resource,
    Resources, RunMode, Schedule, Systems, World, WorldMode,
    app::defaults::{DefaultPhases, DefaultPlugins},
    core::task::{CpuTaskPool, Task},
    ext,
    system::MainWorld,
    world::Archetypes,
};
use std::{
    collections::{HashMap, HashSet},
    hash::Hash,
};

pub mod defaults;
pub mod plugin;
pub mod sync;
pub mod time;

pub use plugin::*;

pub trait AppTag: 'static {
    fn name(&self) -> &'static str {
        ext::short_type_name::<Self>()
    }
}

impl AppTag for Box<dyn AppTag> {
    fn name(&self) -> &'static str {
        self.as_ref().name()
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

#[derive(Phase)]
pub struct Init;

#[derive(Phase)]
pub struct Run;

#[derive(Phase)]
pub struct Start;

#[derive(Phase)]
pub struct PreUpdate;

#[derive(Phase)]
pub struct Update;

#[derive(Phase)]
pub struct PostUpdate;

#[derive(Phase)]
pub struct End;

#[derive(Phase)]
pub struct Extract;

#[derive(Phase)]
pub struct Shutdown;

pub struct App {
    world: World,
    systems: Systems,
}

impl App {
    pub fn empty() -> Self {
        let mut world = World::new();
        let systems = Schedule::new(RunMode::Sequential)
            .build(&mut world)
            .unwrap();

        App { world, systems }
    }

    pub fn new() -> AppBuilder {
        AppBuilder::new()
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

    pub fn run_sub(&mut self, phase: impl Phase, main: MainWorld) {
        self.world.add_resource(main);
        self.run(phase);
        self.world.remove_resource::<MainWorld>();
        self.world.frame = main.frame;
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
    tasks: Vec<Task<App>>,
}

impl Apps {
    pub fn empty() -> Self {
        Self {
            main: App::empty(),
            sub: Vec::new(),
            tasks: Vec::new(),
        }
    }

    fn new(main: App, sub: Vec<App>) -> Self {
        Self {
            main,
            sub,
            tasks: Vec::new(),
        }
    }

    pub fn world(&self) -> &World {
        &self.main.world
    }

    pub fn world_mut(&mut self) -> &mut World {
        &mut self.main.world
    }

    pub fn init(&mut self) {
        self.main.run(Init);
        let main = MainWorld::new(&mut self.main.world);
        for app in &mut self.sub {
            app.run_sub(Init, main);
        }
    }

    pub fn run(&mut self) {
        if self.tasks.len() > 0 {
            smol::block_on(self.await_apps());
        }

        self.main.run(Run);

        let mut tasks = Vec::new();
        let main = MainWorld::new(&mut self.main.world);
        self.sub = self
            .sub
            .drain(..)
            .filter_map(|mut app| {
                app.run_sub(Extract, main);

                if app.is_send() {
                    tasks.push(CpuTaskPool::get().spawn(async move { app.run_once(Run) }));
                    None
                } else {
                    Some(app)
                }
            })
            .collect();

        for app in &mut self.sub {
            app.run(Run);
        }

        self.tasks = tasks;
    }

    async fn await_apps(&mut self) {
        for task in self.tasks.drain(..) {
            self.sub.push(task.await);
        }
    }

    pub fn shutdown(&mut self) {
        self.main.run(Shutdown);
        let main = MainWorld::new(&mut self.main.world);
        for app in &mut self.sub {
            app.run_sub(Shutdown, main);
        }
    }
}

pub struct AppConfig {
    world: World,
    schedule: Schedule,
    plugins: Vec<Box<dyn Plugin>>,
    registered: HashSet<&'static str>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            world: World::new(),
            schedule: Schedule::new(RunMode::Sequential),
            plugins: Vec::new(),
            registered: HashSet::new(),
        }
    }
}

impl AppConfig {
    pub fn new() -> Self {
        Self::default()
    }

    fn with_plugin<P: Plugin>(plugin: P) -> Self {
        let mut config = Self {
            world: World::new(),
            schedule: Schedule::new(RunMode::Sequential),
            plugins: Vec::new(),
            registered: HashSet::new(),
        };

        config.add_plugins(plugin);

        config
    }

    pub fn world(&self) -> &World {
        &self.world
    }

    pub fn world_mut(&mut self) -> &mut World {
        &mut self.world
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

    pub fn register<C: Component>(&mut self) -> &mut Self {
        self.world_mut().register::<C>();
        self
    }

    pub fn register_resource<R: Resource + Send>(&mut self) -> &mut Self {
        self.world_mut().register_resource::<R>();
        self
    }

    pub fn register_non_send_resource<R: Resource>(&mut self) -> &mut Self {
        self.world_mut().register_non_send_resource::<R>();
        self
    }

    pub fn register_event<E: Event>(&mut self) -> &mut Self {
        self.world_mut().register_event::<E>();
        self
    }

    pub fn add_mode<M: WorldMode>(&mut self) -> &mut Self {
        self.world_mut().add_mode::<M>();
        self
    }

    pub fn add_resource<R: Resource + Send>(&mut self, resource: R) -> &mut Self {
        self.world_mut().add_resource(resource);
        self
    }

    pub fn add_non_send_resource<R: Resource>(&mut self, resource: R) -> &mut Self {
        self.world_mut().add_non_send_resource(resource);
        self
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

    pub fn add_systems<M>(
        &mut self,
        phase: impl Phase,
        systems: impl IntoSystemConfigs<M>,
    ) -> &mut Self {
        self.schedule.add_systems(phase, systems);
        self
    }

    pub fn add_phase(&mut self, phase: impl Phase) -> &mut Self {
        self.schedule.add_phase(phase);
        self
    }

    pub fn add_sub_phase(&mut self, main: impl Phase, sub: impl Phase) -> &mut Self {
        self.schedule.add_sub_phase(main, sub);
        self
    }

    /// Run the `phase` before the `target` phase.
    pub fn run_before(&mut self, phase: impl Phase, target: impl Phase) -> &mut Self {
        self.schedule.run_before(phase, target);
        self
    }

    /// Run the `phase` after the `target` phase.
    pub fn run_after(&mut self, phase: impl Phase, target: impl Phase) -> &mut Self {
        self.schedule.run_after(phase, target);
        self
    }

    pub fn add_plugins<P: PluginKit>(&mut self, plugins: P) -> &mut Self {
        plugins.get(self);
        self
    }

    pub fn build(mut self, main: Option<MainWorld>) -> App {
        AppBuilder::from_sub_app(&mut self).build_app(main)
    }
}

impl PluginCollection for AppConfig {
    fn add_plugin<P: Plugin>(&mut self, plugin: P) -> &mut Self {
        let mut builder = AppBuilder::from_sub_app(self);
        builder.add_plugins(plugin);

        std::mem::swap(self, &mut builder.main);

        self
    }
}

pub enum AppRunner {
    Run(Box<dyn Fn(Apps) -> Apps + 'static>),
    BuildAndRun(Box<dyn Fn(AppBuilder) -> Apps + 'static>),
}

pub struct AppBuilder {
    main: AppConfig,
    sub_apps: HashMap<Box<dyn AppTag>, AppConfig>,
    runner: Option<AppRunner>,
}

impl Default for AppBuilder {
    fn default() -> Self {
        let mut main = AppConfig::new();
        main.add_plugins(DefaultPlugins::default());

        Self {
            main,
            sub_apps: HashMap::new(),
            runner: None,
        }
    }
}

impl AppBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    fn from_sub_app(sub_app: &mut AppConfig) -> Self {
        Self {
            main: std::mem::take(sub_app),
            sub_apps: Default::default(),
            runner: None,
        }
    }

    pub fn world(&self) -> &World {
        &self.main.world
    }

    pub fn world_mut(&mut self) -> &mut World {
        &mut self.main.world
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

    pub fn register<C: Component>(&mut self) -> &mut Self {
        self.world_mut().register::<C>();
        self
    }

    pub fn register_resource<R: Resource + Send>(&mut self) -> &mut Self {
        self.world_mut().register_resource::<R>();
        self
    }

    pub fn register_non_send_resource<R: Resource>(&mut self) -> &mut Self {
        self.world_mut().register_non_send_resource::<R>();
        self
    }

    pub fn register_event<E: Event>(&mut self) -> &mut Self {
        self.world_mut().register_event::<E>();
        self
    }

    pub fn add_mode<M: WorldMode>(&mut self) -> &mut Self {
        self.world_mut().add_mode::<M>();
        self
    }

    pub fn add_resource<R: Resource + Send>(&mut self, resource: R) -> &mut Self {
        self.world_mut().add_resource(resource);
        self
    }

    pub fn add_non_send_resource<R: Resource>(&mut self, resource: R) -> &mut Self {
        self.world_mut().add_non_send_resource(resource);
        self
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

    pub fn add_systems<M>(
        &mut self,
        phase: impl Phase,
        systems: impl IntoSystemConfigs<M>,
    ) -> &mut Self {
        self.main.schedule.add_systems(phase, systems);
        self
    }

    pub fn add_phase(&mut self, phase: impl Phase) -> &mut Self {
        self.main.schedule.add_phase(phase);
        self
    }

    pub fn add_sub_phase(&mut self, main: impl Phase, sub: impl Phase) -> &mut Self {
        self.main.schedule.add_sub_phase(main, sub);
        self
    }

    pub fn run_before(&mut self, phase: impl Phase, target: impl Phase) -> &mut Self {
        self.main.schedule.run_before(phase, target);
        self
    }

    pub fn run_after(&mut self, phase: impl Phase, target: impl Phase) -> &mut Self {
        self.main.schedule.run_after(phase, target);
        self
    }

    pub fn add_plugins<P: PluginKit>(&mut self, plugins: P) -> &mut Self {
        plugins.get(self);
        self
    }

    pub fn sub_app(&self, app: impl AppTag) -> Option<&AppConfig> {
        self.sub_apps.get(&(Box::new(app) as Box<dyn AppTag>))
    }

    pub fn sub_app_mut(&mut self, app: impl AppTag) -> &mut AppConfig {
        self.sub_apps
            .entry(Box::new(app))
            .or_insert(AppConfig::with_plugin(DefaultPhases::<false>::default()))
    }

    pub fn scoped_sub_app(
        &mut self,
        app: impl AppTag,
        f: impl FnOnce(&mut AppConfig),
    ) -> &mut Self {
        let app = self.sub_app_mut(app);
        f(app);
        self
    }

    pub fn set_runner(&mut self, runner: impl Fn(Apps) -> Apps + 'static) -> &mut Self {
        self.runner = Some(AppRunner::Run(Box::new(runner)));
        self
    }

    pub fn set_builder(&mut self, builder: impl Fn(AppBuilder) -> Apps + 'static) -> &mut Self {
        self.runner = Some(AppRunner::BuildAndRun(Box::new(builder)));
        self
    }

    pub fn run(&mut self) -> Apps {
        let runner = self.runner.take();

        match runner {
            Some(AppRunner::Run(f)) => f(self.build()),
            Some(AppRunner::BuildAndRun(f)) => f(std::mem::take(self)),
            None => Self::default_runner(self.build()),
        }
    }

    pub fn build(&mut self) -> Apps {
        let mut builder = std::mem::take(self);
        let mut main = builder.build_app(None);
        let main_world = MainWorld::new(&mut main.world);
        let sub_apps = builder
            .sub_apps
            .into_values()
            .map(|config| config.build(Some(main_world)))
            .collect::<Vec<_>>();

        Apps::new(main, sub_apps)
    }

    fn build_app(&mut self, main: Option<MainWorld>) -> App {
        let mut plugins = std::mem::take(&mut self.main.plugins);
        let mut queue: Vec<Box<dyn Plugin>> = vec![];

        while !plugins.is_empty() {
            for plugin in &mut plugins {
                plugin.build(self);
            }

            if self.main.plugins.is_empty() {
                for mut plugin in plugins.drain(..) {
                    plugin.finish(self);
                }

                if self.main.plugins.is_empty() {
                    for mut plugin in queue.drain(..) {
                        plugin.finish(self);
                    }
                }
            } else {
                queue.append(&mut plugins);
            }

            plugins.append(&mut self.main.plugins);
        }

        let AppConfig {
            mut world,
            schedule,
            ..
        } = std::mem::take(&mut self.main);

        if let Some(main) = main {
            world.add_resource(main);
        }

        let systems = schedule.build(&mut world).unwrap();
        world.remove_resource::<MainWorld>();

        App { world, systems }
    }

    fn default_runner(mut apps: Apps) -> Apps {
        apps.init();
        apps.run();
        apps.shutdown();
        smol::block_on(apps.await_apps());

        apps
    }
}

impl PluginCollection for AppBuilder {
    fn add_plugin<P: Plugin>(&mut self, mut plugin: P) -> &mut Self {
        if !self.main.registered.contains(plugin.name()) {
            self.main.registered.insert(plugin.name());

            plugin.setup(self);
            self.main.plugins.push(Box::new(plugin));
        }

        self
    }
}

#[allow(unused_imports, dead_code)]
mod tests {
    use super::{App, Plugin, PluginCollection};
    use crate::{AppTag, Resource, app::AppConfig};

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Resource)]
    pub struct Value(usize);

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

        fn build(&mut self, app: &mut super::AppBuilder) {
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

    #[derive(AppTag)]
    pub struct TestApp;

    #[test]
    fn build() {
        let mut config = AppConfig::new();
        config.add_resource(Value(0));
        config.add_plugin(PluginA);
        let app = config.build(None);

        assert_eq!(app.world.resource::<Value>().0, 4);
    }

    #[test]
    fn build_multi_app() {
        let mut builder = App::new();
        builder.add_resource(Value(0));
        builder.add_plugins(PluginA);
        builder.sub_app_mut(TestApp).add_resource(Value(0));
        builder.sub_app_mut(TestApp).add_plugins(PluginA);

        let apps = builder.run();

        assert_eq!(apps.world().resource::<Value>().0, 4);
        assert_eq!(apps.sub[0].world().resource::<Value>().0, 4);
    }
}
