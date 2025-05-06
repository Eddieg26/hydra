use std::{
    collections::{HashMap, HashSet},
    hash::Hash,
    thread::JoinHandle,
};

use crate::{Phase, Resource, RunMode, Schedule, Systems, World, WorldCell, ext};

#[allow(unused_variables)]
pub trait Plugin: 'static {
    fn name(&self) -> &'static str {
        ext::short_type_name::<Self>()
    }

    /// Setup is called when the plugin is added to the app.
    /// It is used to register systems, resources, and other app components.
    fn setup(&mut self, app: &mut AppBuilder) {}

    /// Run is called when the app is being built.
    fn run(&mut self, app: &mut AppBuilder);

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

pub struct AppConfig {
    world: World,
    schedule: Schedule,
    plugins: Vec<Box<dyn Plugin>>,
    registered: HashSet<&'static str>,
}

impl AppConfig {
    pub fn new() -> Self {
        Self {
            world: World::new(),
            schedule: Schedule::new(RunMode::Sequential),
            plugins: Vec::new(),
            registered: HashSet::new(),
        }
    }
}

pub enum AppConfigKind {
    Main {
        config: AppConfig,
        secondary: HashMap<Box<dyn AppTag>, AppConfig>,
        runner: Option<Box<dyn Fn(Apps) -> Apps>>,
    },
    Sub(AppConfig),
}

pub struct AppBuilder(AppConfigKind);
impl AppBuilder {
    pub fn new() -> Self {
        Self(AppConfigKind::Main {
            config: AppConfig::new(),
            secondary: HashMap::new(),
            runner: None,
        })
    }

    pub fn single() -> Self {
        Self(AppConfigKind::Sub(AppConfig::new()))
    }

    pub fn world(&self) -> &World {
        &self.config().world
    }

    pub fn world_mut(&mut self) -> &mut World {
        &mut self.config_mut().world
    }

    pub fn add_sub_app(&mut self, app: impl AppTag) {
        let app = Box::new(app) as Box<dyn AppTag>;
        match &mut self.0 {
            AppConfigKind::Main { secondary, .. } => {
                secondary.entry(app).or_insert(AppConfig::new());
            }
            AppConfigKind::Sub(_) => panic!("Cannot add sub app to a sub app"),
        }
    }

    pub fn sub_app(&self, app: impl AppTag) -> Option<&AppConfig> {
        match &self.0 {
            AppConfigKind::Main { secondary, .. } => {
                secondary.get(&(Box::new(app) as Box<dyn AppTag>))
            }
            AppConfigKind::Sub(_) => None,
        }
    }

    pub fn sub_app_mut(&mut self, app: impl AppTag) -> Option<&mut AppConfig> {
        match &mut self.0 {
            AppConfigKind::Main { secondary, .. } => {
                secondary.get_mut(&(Box::new(app) as Box<dyn AppTag>))
            }
            AppConfigKind::Sub(_) => None,
        }
    }

    pub fn add_plugins<P: PluginKit>(&mut self, plugins: P) {
        plugins.get(self);
    }

    pub fn build(self) -> App {
        match self.0 {
            AppConfigKind::Main {
                config,
                secondary,
                runner,
            } => {
                let mut main = Self(AppConfigKind::Main {
                    config,
                    secondary,
                    runner,
                });

                main.build_plugins();

                let AppConfigKind::Main {
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
                    .map(|app| {
                        let mut app = Self(AppConfigKind::Sub(app));
                        app.build_plugins();

                        App::from(app.into_config())
                    })
                    .collect();

                match runner {
                    Some(runner) => runner(Apps::new(main, sub)).main,
                    None => Self::default_runner(Apps::new(main, sub)).main,
                }
            }
            AppConfigKind::Sub(config) => {
                let mut sub = Self(AppConfigKind::Sub(config));
                sub.build_plugins();

                App::from(sub.into_config())
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

    fn config(&self) -> &AppConfig {
        match &self.0 {
            AppConfigKind::Main { config, .. } => config,
            AppConfigKind::Sub(config) => config,
        }
    }

    fn config_mut(&mut self) -> &mut AppConfig {
        match &mut self.0 {
            AppConfigKind::Main { config, .. } => config,
            AppConfigKind::Sub(config) => config,
        }
    }

    fn plugins(&self) -> &Vec<Box<dyn Plugin>> {
        match &self.0 {
            AppConfigKind::Main { config, .. } => &config.plugins,
            AppConfigKind::Sub(config) => &config.plugins,
        }
    }

    fn plugins_mut(&mut self) -> &mut Vec<Box<dyn Plugin>> {
        match &mut self.0 {
            AppConfigKind::Main { config, .. } => &mut config.plugins,
            AppConfigKind::Sub(config) => &mut config.plugins,
        }
    }

    fn into_config(self) -> AppConfig {
        match self.0 {
            AppConfigKind::Main { config, .. } => config,
            AppConfigKind::Sub(config) => config,
        }
    }

    fn default_runner(mut apps: Apps) -> Apps {
        apps.init();
        apps.run();
        apps.shutdown();

        apps
    }
}

impl Plugins for AppBuilder {
    fn add_plugin<P: Plugin>(&mut self, mut plugin: P) -> &mut Self {
        if !self.config().registered.contains(plugin.name()) {
            self.config_mut().registered.insert(plugin.name());

            plugin.setup(self);
            self.plugins_mut().push(Box::new(plugin));
        }

        self
    }
}

impl From<AppConfig> for App {
    fn from(mut value: AppConfig) -> Self {
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
    handles: Vec<JoinHandle<App>>,
}

impl Apps {
    fn new(main: App, sub: Vec<App>) -> Self {
        Self {
            main,
            sub,
            handles: Vec::new(),
        }
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
