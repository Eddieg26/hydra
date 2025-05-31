use crate::{
    Component, Components, Entities, Event, EventRegistry, IntoSystemConfigs, Phase, Resource,
    Resources, RunMode, Schedule, Systems, World, WorldMode,
    core::task::{CpuTaskPool, Task, TaskPoolSettings},
    ext,
    world::{Archetypes, WorldCell},
};
use std::{
    collections::{HashMap, HashSet},
    hash::Hash,
};

#[allow(unused_variables)]
pub trait Plugin: 'static {
    fn name(&self) -> &'static str {
        ext::short_type_name::<Self>()
    }

    /// Setup is called when the plugin is added to the app.
    /// It is used to register systems, resources, and other app components.
    fn setup(&mut self, app: &mut AppBuilder);

    fn build(&mut self, app: &mut AppBuilder) {}

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
pub struct Update;

#[derive(Phase)]
pub struct End;

#[derive(Phase)]
pub struct Extract;

#[derive(Phase)]
pub struct Shutdown;

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

    pub fn add_main_phases(&mut self) {
        self.schedule.add_phase(Init);
        self.schedule.add_phase(Run);
        self.schedule.add_sub_phase(Run, Start);
        self.schedule.add_sub_phase(Run, Update);
        self.schedule.add_sub_phase(Run, End);
        self.schedule.add_phase(Shutdown);
    }

    pub fn add_sub_phases(&mut self) {
        self.schedule.add_phase(Run);
        self.schedule.add_phase(Extract);
    }
}

pub enum AppType {
    Main {
        config: AppBuildInfo,
        secondary: HashMap<Box<dyn AppTag>, AppBuilder>,
        runner: Option<Box<dyn Fn(Apps) -> Apps>>,
        task_pool_settings: TaskPoolSettings,
        building: bool,
    },
    Sub {
        config: AppBuildInfo,
        task_pool_settings: TaskPoolSettings,
        building: bool,
    },
}

impl Default for AppType {
    fn default() -> Self {
        Self::Main {
            config: AppBuildInfo::new(),
            secondary: HashMap::new(),
            runner: None,
            task_pool_settings: TaskPoolSettings::default(),
            building: false,
        }
    }
}

pub struct AppBuilder(AppType);
impl AppBuilder {
    pub fn new() -> Self {
        let mut config = AppBuildInfo::new();
        config.add_main_phases();

        Self(AppType::Main {
            config,
            secondary: HashMap::new(),
            runner: None,
            task_pool_settings: TaskPoolSettings::default(),
            building: false,
        })
    }

    pub fn world(&self) -> &World {
        &self.info().world
    }

    pub fn world_mut(&mut self) -> &mut World {
        &mut self.info_mut().world
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
        self.info_mut().schedule.add_systems(phase, systems);
        self
    }

    pub fn add_phase(&mut self, phase: impl Phase) -> &mut Self {
        self.info_mut().schedule.add_phase(phase);
        self
    }

    pub fn add_sub_phase(&mut self, phase: impl Phase, target: impl Phase) -> &mut Self {
        self.info_mut().schedule.add_sub_phase(phase, target);
        self
    }

    pub fn run_before(&mut self, phase: impl Phase, target: impl Phase) -> &mut Self {
        self.info_mut().schedule.run_before(phase, target);
        self
    }

    pub fn run_after(&mut self, phase: impl Phase, target: impl Phase) -> &mut Self {
        self.info_mut().schedule.run_after(phase, target);
        self
    }

    pub fn task_pool_settings(&self) -> &TaskPoolSettings {
        match &self.0 {
            AppType::Main {
                task_pool_settings, ..
            } => task_pool_settings,
            AppType::Sub {
                task_pool_settings, ..
            } => task_pool_settings,
        }
    }

    pub fn task_pool_settings_mut(&mut self) -> &mut TaskPoolSettings {
        match &mut self.0 {
            AppType::Main {
                task_pool_settings, ..
            } => task_pool_settings,
            AppType::Sub {
                task_pool_settings, ..
            } => task_pool_settings,
        }
    }

    pub fn add_sub_app(&mut self, app: impl AppTag) -> &mut AppBuilder {
        let app = Box::new(app) as Box<dyn AppTag>;
        match &mut self.0 {
            AppType::Main { secondary, .. } => {
                let mut config = AppBuildInfo::new();
                config.add_sub_phases();

                secondary.entry(app).or_insert(AppBuilder(AppType::Sub {
                    config,
                    task_pool_settings: TaskPoolSettings::default(),
                    building: false,
                }))
            }
            AppType::Sub { .. } => panic!("Cannot add sub app to a sub app"),
        }
    }

    pub fn sub_app(&self, app: impl AppTag) -> Option<&AppBuilder> {
        match &self.0 {
            AppType::Main { secondary, .. } => secondary.get(&(Box::new(app) as Box<dyn AppTag>)),
            AppType::Sub { .. } => None,
        }
    }

    pub fn sub_app_mut(&mut self, app: impl AppTag) -> Option<&mut AppBuilder> {
        match &mut self.0 {
            AppType::Main { secondary, .. } => {
                secondary.get_mut(&(Box::new(app) as Box<dyn AppTag>))
            }
            AppType::Sub { .. } => None,
        }
    }

    pub fn add_plugins<P: PluginKit>(&mut self, plugins: P) -> &mut Self {
        plugins.get(self);
        self
    }

    pub fn build(&mut self) -> Apps {
        if self.is_building() {
            panic!("AppBuilder is already building. You cannot call build() multiple times.");
        }

        match std::mem::take(&mut self.0) {
            AppType::Main {
                config,
                secondary,
                runner,
                task_pool_settings,
                ..
            } => {
                let mut main = Self(AppType::Main {
                    config,
                    secondary,
                    runner,
                    task_pool_settings,
                    building: true,
                });

                main.build_plugins();

                let AppType::Main {
                    config,
                    secondary,
                    runner,
                    task_pool_settings,
                    ..
                } = main.0
                else {
                    panic!("Expected AppConfigKind::Main");
                };

                task_pool_settings.init_task_pools();

                let main = App::from(config);
                let sub = secondary
                    .into_values()
                    .map(|mut app| {
                        app.info_mut().add_sub_phases();
                        app.build_plugins();

                        App::from(app.into_build_info())
                    })
                    .collect::<Vec<_>>();

                match runner {
                    Some(runner) => runner(Apps::new(main, sub)),
                    None => Self::default_runner(Apps::new(main, sub)),
                }
            }
            AppType::Sub {
                config,
                task_pool_settings,
                ..
            } => {
                let mut builder = Self(AppType::Sub {
                    config,
                    task_pool_settings,
                    building: true,
                });

                builder.build_plugins();
                builder.task_pool_settings().init_task_pools();

                let app = App::from(builder.into_build_info());
                Apps::new(app, vec![])
            }
        }
    }

    fn is_building(&self) -> bool {
        match &self.0 {
            AppType::Main { building, .. } => *building,
            AppType::Sub { building, .. } => *building,
        }
    }

    fn build_plugins(&mut self) {
        let mut plugins = std::mem::take(self.plugins_mut());
        let mut queue: Vec<Box<dyn Plugin>> = vec![];
        while !plugins.is_empty() {
            for plugin in &mut plugins {
                plugin.build(self);
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
            AppType::Sub { config, .. } => config,
        }
    }

    fn info_mut(&mut self) -> &mut AppBuildInfo {
        match &mut self.0 {
            AppType::Main { config, .. } => config,
            AppType::Sub { config, .. } => config,
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
            AppType::Sub { config, .. } => config,
        }
    }

    fn default_runner(mut apps: Apps) -> Apps {
        apps.init();
        apps.run();
        apps.shutdown();
        smol::block_on(apps.await_apps());

        apps
    }
}

impl Default for AppBuilder {
    fn default() -> Self {
        Self::new()
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
    tasks: Vec<Task<App>>,
}

impl Apps {
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

    pub fn init(&mut self) {
        self.main.run(Init);
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
                app.extract(main);

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
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Resource)]
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

unsafe impl Send for MainWorld {}
unsafe impl Sync for MainWorld {}

#[allow(unused_imports, dead_code)]
mod tests {
    use super::{App, Plugin, Plugins};
    use crate::{AppTag, Resource};

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
    fn build_plugins() {
        let mut builder = App::new();
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
