use crate::{
    core::{AccessBitset, Frame, SparseIndex},
    world::{ComponentId, ResourceId, World, cell::WorldCell},
};
use std::{any::Any, borrow::Cow, cell::UnsafeCell, collections::HashSet};

pub mod arg;
pub mod executor;
pub mod query;
pub mod schedule;

pub use arg::*;
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Access {
    Read,
    Write,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SystemAccess {
    Component { id: ComponentId, access: Access },
    Resource { id: ResourceId, access: Access },
}

impl SystemAccess {
    pub fn resource(id: ResourceId, access: Access) -> Self {
        SystemAccess::Resource { id, access }
    }

    pub fn component(id: ComponentId, access: Access) -> Self {
        SystemAccess::Component { id, access }
    }
}

pub struct SystemMeta {
    pub id: SystemId,
    pub name: Option<SystemName>,
    /// Components that the system accesses.
    pub components: AccessBitset,
    /// Resources that the system accesses.
    pub resources: AccessBitset,
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
            components: AccessBitset::new(),
            resources: AccessBitset::new(),
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
    init: fn(&mut World) -> Box<dyn Any + Send + Sync>,
    access: fn(&Box<dyn Any + Send + Sync>) -> Vec<SystemAccess>,
    run: SystemRun,
    apply: SystemApply,
}

impl SystemConfig {
    pub fn into_system_node(self, world: &mut World) -> SystemNode {
        let state = (self.init)(world);
        let mut components = AccessBitset::with_capacity(world.components().len());
        let mut resources = AccessBitset::with_capacity(world.resources().len());

        for access in (self.access)(&state) {
            match access {
                SystemAccess::Component { id, access } => match access {
                    Access::Read => components.read(id.to_usize()),
                    Access::Write => components.write(id.to_usize()),
                },
                SystemAccess::Resource { id, access } => match access {
                    Access::Read => resources.read(id.to_usize()),
                    Access::Write => resources.write(id.to_usize()),
                },
            };
        }

        let meta = SystemMeta {
            id: self.id,
            name: self.name,
            components,
            resources,
            send: self.send,
            exclusive: self.exclusive,
            frame: Frame::ZERO,
        };

        SystemNode {
            system: System::new(meta, state, self.run, self.apply),
            dependencies: self.dependencies,
        }
    }
}

pub struct SystemNode {
    pub system: System,
    pub dependencies: HashSet<SystemId>,
}

impl SystemNode {
    pub fn has_dependency(&self, other: &SystemNode) -> bool {
        self.dependencies.contains(&other.system.meta.id)
            || self
                .system
                .meta
                .components
                .conflicts(&other.system.meta.components)
            || self
                .system
                .meta
                .resources
                .conflicts(&other.system.meta.resources)
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

impl<F: Fn() + Send + Sync + 'static> IntoSystemConfigs<()> for F {
    fn configs(self) -> SystemConfigs {
        SystemConfigs::Config(SystemConfig {
            id: SystemId::new(),
            name: None,
            exclusive: false,
            send: true,
            dependencies: HashSet::new(),
            init: |_| Box::new(()),
            access: |_| vec![],
            run: Box::new(move |_, _, _| {
                self();
            }),
            apply: Box::new(|_, _| {}),
        })
    }

    fn before<Marker>(self, configs: impl IntoSystemConfigs<Marker>) -> SystemConfigs {
        self.configs().before(configs)
    }
}

pub type SystemState = Box<dyn Any + Send + Sync>;
pub type SystemRun =
    Box<dyn Fn(&mut Box<dyn Any + Send + Sync>, WorldCell, &SystemMeta) + Send + Sync>;
pub type SystemApply = Box<dyn Fn(&mut Box<dyn Any + Send + Sync>, &mut World) + Send + Sync>;

pub struct System {
    meta: SystemMeta,
    state: SystemState,
    run: SystemRun,
    apply: SystemApply,
}

impl System {
    pub fn new(meta: SystemMeta, state: SystemState, run: SystemRun, apply: SystemApply) -> Self {
        Self {
            meta,
            state,
            run,
            apply,
        }
    }

    pub fn run(&mut self, world: WorldCell) {
        (self.run)(&mut self.state, world, &self.meta);
        self.meta.frame = unsafe { world.get().frame() }
    }

    pub fn apply(&mut self, world: &mut World) {
        (self.apply)(&mut self.state, world);
    }
}

impl From<SystemNode> for System {
    fn from(value: SystemNode) -> Self {
        value.system
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
        Self(UnsafeCell::new(node.system))
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
