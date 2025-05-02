use crate::{
    AccessError, WorldAccess,
    core::Frame,
    world::{World, cell::WorldCell},
};
use fixedbitset::FixedBitSet;
use std::{any::Any, borrow::Cow, cell::UnsafeCell, collections::HashSet};

pub mod arg;
pub mod commands;
pub mod executor;
pub mod query;
pub mod schedule;

pub use arg::*;
pub use commands::*;
pub use executor::*;
pub use query::*;
pub use schedule::*;

pub type SystemName = Cow<'static, str>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SystemId(u32);
impl SystemId {
    fn new() -> Self {
        static mut ID: u32 = 0;
        unsafe {
            let id = ID;
            ID += 1;
            SystemId(id)
        }
    }
}

pub struct SystemMeta {
    pub id: SystemId,
    pub name: Option<SystemName>,
    /// Components that the system accesses.
    pub components: FixedBitSet,
    /// Resources that the system accesses.
    pub resources: FixedBitSet,
    /// The system contains only send resources.
    pub send: bool,
    /// The system should be ran exclusively in the given frame.
    pub exclusive: bool,
    /// The frame in which the system was last executed.
    pub frame: Frame,
}

impl Default for SystemMeta {
    fn default() -> Self {
        Self {
            id: SystemId::new(),
            name: None,
            components: FixedBitSet::new(),
            resources: FixedBitSet::new(),
            send: true,
            exclusive: false,
            frame: Frame::ZERO,
        }
    }
}

pub struct SystemConfig {
    id: SystemId,
    name: Option<SystemName>,
    exclusive: bool,
    send: bool,
    dependencies: HashSet<SystemId>,
    init: SystemInitFn,
    run: SystemRunFn,
    update: SystemUpdateFn,
}

impl SystemConfig {
    pub fn into_system_node(self, world: &mut World) -> SystemNode {
        let mut access = WorldAccess::new();
        let state = (self.init)(world, &mut access);

        if let Err(error) = access.validate() {
            match error {
                AccessError::Component(id) => {
                    let component = world.components().get_by_id(id).unwrap();
                    panic!(
                        "Invalid component access: {} for system: {:?}",
                        component.name(),
                        self.name
                    );
                }
                AccessError::Resource(id) => {
                    let resource = world.resources().get_meta(id).unwrap();
                    panic!(
                        "Invalid resource access: {} for system: {:?}",
                        resource.name(),
                        self.name
                    );
                }
            }
        }

        SystemNode {
            config: self,
            state,
            access,
        }
    }
}

pub struct SystemNode {
    pub config: SystemConfig,
    pub state: SystemState,
    pub access: WorldAccess,
}

impl SystemNode {
    pub fn has_dependency(&self, other: &SystemNode) -> bool {
        if self.config.dependencies.contains(&other.config.id) {
            return true;
        }

        self.access.conflicts(&other.access).is_err()
    }
}

pub enum SystemConfigs {
    Config(SystemConfig),
    Configs(Vec<SystemConfig>),
}

impl SystemConfigs {
    pub fn single(self) -> SystemConfig {
        match self {
            SystemConfigs::Config(config) => config,
            SystemConfigs::Configs(configs) => configs.into_iter().next().unwrap(),
        }
    }

    pub fn flatten(self) -> Vec<SystemConfig> {
        match self {
            SystemConfigs::Config(config) => vec![config],
            SystemConfigs::Configs(configs) => configs,
        }
    }
}

impl AsRef<SystemConfig> for SystemConfigs {
    fn as_ref(&self) -> &SystemConfig {
        match self {
            SystemConfigs::Config(config) => config,
            SystemConfigs::Configs(configs) => &configs[0],
        }
    }
}

impl AsMut<SystemConfig> for SystemConfigs {
    fn as_mut(&mut self) -> &mut SystemConfig {
        match self {
            SystemConfigs::Config(config) => config,
            SystemConfigs::Configs(configs) => &mut configs[0],
        }
    }
}

impl SystemConfigs {
    pub fn new(config: SystemConfig) -> Self {
        SystemConfigs::Config(config)
    }

    pub fn configs(configs: Vec<SystemConfig>) -> Self {
        SystemConfigs::Configs(configs)
    }

    pub fn config(&self) -> &SystemConfig {
        match self {
            SystemConfigs::Config(config) => config,
            SystemConfigs::Configs(configs) => &configs[0],
        }
    }
}

pub trait IntoSystemConfig<M> {
    type In;

    fn config(self) -> SystemConfig;
}

impl<M, I: IntoSystemConfig<M>> IntoSystemConfigs<M> for I {
    fn configs(self) -> SystemConfigs {
        SystemConfigs::Config(self.config())
    }

    fn before<Marker>(self, configs: impl IntoSystemConfigs<Marker>) -> SystemConfigs {
        self.configs().before(configs)
    }
}

pub trait IntoSystemConfigs<M> {
    fn configs(self) -> SystemConfigs;
    fn before<Marker>(self, configs: impl IntoSystemConfigs<Marker>) -> SystemConfigs;
    fn after<Marker>(self, configs: impl IntoSystemConfigs<Marker>) -> SystemConfigs
    where
        Self: Sized,
    {
        configs.before(self)
    }
}

impl IntoSystemConfigs<()> for SystemConfigs {
    fn configs(self) -> SystemConfigs {
        self
    }

    fn before<Marker>(self, configs: impl IntoSystemConfigs<Marker>) -> SystemConfigs {
        let configs = configs.configs();

        match (self, configs) {
            (SystemConfigs::Config(before), SystemConfigs::Config(mut after)) => {
                after.dependencies.insert(before.id);
                Self::Configs(vec![before, after])
            }
            (SystemConfigs::Config(before), SystemConfigs::Configs(mut after)) => {
                after.iter_mut().for_each(|s| {
                    s.dependencies.insert(before.id);
                });
                after.insert(0, before);
                Self::Configs(after)
            }
            (SystemConfigs::Configs(mut before), SystemConfigs::Config(mut after)) => {
                after.dependencies.extend(before.iter().map(|s| s.id));
                before.push(after);
                Self::Configs(before)
            }
            (SystemConfigs::Configs(mut before), SystemConfigs::Configs(mut after)) => {
                after
                    .iter_mut()
                    .for_each(|s| s.dependencies.extend(before.iter().map(|s| s.id)));
                before.extend(after);
                Self::Configs(before)
            }
        }
    }
}

impl<F: Fn() + Send + Sync + 'static> IntoSystemConfig<()> for F {
    type In = ();

    fn config(self) -> SystemConfig {
        SystemConfig {
            id: SystemId::new(),
            name: None,
            exclusive: false,
            send: true,
            dependencies: HashSet::new(),
            init: |_, _| Box::new(()),
            run: Box::new(move |_, _, _| {
                self();
            }),
            update: Box::new(|_, _| {}),
        }
    }
}

pub type SystemState = Box<dyn Any + Send + Sync>;
pub type SystemInitFn = fn(&mut World, &mut WorldAccess) -> Box<dyn Any + Send + Sync>;
pub type SystemRunFn =
    Box<dyn Fn(&mut Box<dyn Any + Send + Sync>, WorldCell, &SystemMeta) + Send + Sync>;
pub type SystemUpdateFn = Box<dyn Fn(&mut Box<dyn Any + Send + Sync>, &mut World) + Send + Sync>;

pub struct System {
    meta: SystemMeta,
    state: SystemState,
    run: SystemRunFn,
    update: SystemUpdateFn,
}

impl System {
    pub fn new(
        meta: SystemMeta,
        state: SystemState,
        run: SystemRunFn,
        update: SystemUpdateFn,
    ) -> Self {
        Self {
            meta,
            state,
            run,
            update,
        }
    }

    pub fn run(&mut self, world: WorldCell) {
        (self.run)(&mut self.state, world, &self.meta);
        self.meta.frame = unsafe { world.get().frame() }
    }

    pub fn update(&mut self, world: &mut World) {
        (self.update)(&mut self.state, world);
    }
}

impl From<SystemNode> for System {
    fn from(value: SystemNode) -> Self {
        let resources = value.access.resources.collect();
        let mut components = value.access.components.collect();
        for archetype in value.access.archetypes {
            components.union_with(archetype.required().get_read());
            components.union_with(archetype.required().get_write());
        }

        let meta = SystemMeta {
            id: value.config.id,
            name: value.config.name,
            components,
            resources,
            send: value.config.send,
            exclusive: value.config.exclusive,
            frame: Frame::ZERO,
        };

        System::new(meta, value.state, value.config.run, value.config.update)
    }
}

pub struct SystemCell(UnsafeCell<System>);

impl From<System> for SystemCell {
    fn from(system: System) -> Self {
        Self(UnsafeCell::new(system))
    }
}

impl From<SystemNode> for SystemCell {
    fn from(node: SystemNode) -> Self {
        Self(UnsafeCell::new(node.into()))
    }
}

impl SystemCell {
    pub fn get(&self) -> &System {
        unsafe { self.0.get().as_ref().unwrap() }
    }

    pub fn get_mut(&mut self) -> &mut System {
        self.0.get_mut()
    }

    /// The caller must ensure that the system is not borrowed elsewhere.
    pub unsafe fn cast_mut(&self) -> &mut System {
        unsafe { &mut *self.0.get() }
    }
}

unsafe impl Send for SystemCell {}
unsafe impl Sync for SystemCell {}
