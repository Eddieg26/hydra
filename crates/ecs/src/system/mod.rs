use crate::{
    core::{Frame, SparseIndex},
    world::{ComponentId, ResourceId, World, cell::WorldCell},
};
use std::{any::Any, borrow::Cow, cell::UnsafeCell, collections::HashSet};
use fixedbitset::FixedBitSet;

pub mod arg;
pub mod commands;
pub mod executor;
pub mod query;
pub mod query_v2;
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

#[derive(Clone)]
pub struct FullAccess<I: SparseIndex> {
    pub(crate) read: FixedBitSet,
    pub(crate) write: FixedBitSet,
    pub(crate) include: FixedBitSet,
    pub(crate) exclude: FixedBitSet,
    _marker: std::marker::PhantomData<I>,
}

impl<I: SparseIndex> FullAccess<I> {
    pub fn new() -> Self {
        Self {
            read: FixedBitSet::new(),
            write: FixedBitSet::new(),
            include: FixedBitSet::new(),
            exclude: FixedBitSet::new(),
            _marker: Default::default(),
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            read: FixedBitSet::with_capacity(capacity),
            write: FixedBitSet::with_capacity(capacity),
            include: FixedBitSet::with_capacity(capacity),
            exclude: FixedBitSet::with_capacity(capacity),
            _marker: Default::default(),
        }
    }

    pub fn read(&mut self, id: I) -> I {
        self.read.grow(id.to_usize() + 1);
        self.read.set(id.to_usize(), true);
        id
    }

    pub fn write(&mut self, id: I) -> I {
        self.write.grow(id.to_usize() + 1);
        self.write.set(id.to_usize(), true);
        id
    }

    pub fn include(&mut self, id: I) -> I {
        self.include.grow(id.to_usize() + 1);
        self.include.set(id.to_usize(), true);
        id
    }

    pub fn exclude(&mut self, id: I) -> I {
        self.include.grow(id.to_usize() + 1);
        self.include.set(id.to_usize(), true);
        id
    }

    pub fn conflicts(&self, other: &Self) -> Result<(), usize> {
        if !self.exclude.is_disjoint(&other.read)
            || !self.exclude.is_disjoint(&other.write)
            || !self.exclude.is_disjoint(&other.include)
        {
            return Ok(());
        }

        if let Some(conflict) = self.read.intersection(&other.write).next() {
            return Err(conflict);
        }

        if let Some(conflict) = self.write.intersection(&other.write).next() {
            return Err(conflict);
        }

        if let Some(conflict) = self.write.intersection(&other.read).next() {
            return Err(conflict);
        }

        Ok(())
    }

    pub fn validate(&self) -> Result<(), usize> {
        if let Some(index) = self.read.intersection(&self.write).next() {
            return Err(index);
        }

        if let Some(index) = self.read.intersection(&self.exclude).next() {
            return Err(index);
        }

        if let Some(index) = self.write.intersection(&self.exclude).next() {
            return Err(index);
        }

        Ok(())
    }
}

pub struct Access<I: SparseIndex>(FixedBitSet, std::marker::PhantomData<I>);
impl<I: SparseIndex> Access<I> {
    pub fn new() -> Self {
        Self(FixedBitSet::new(), Default::default())
    }

    pub fn merge(&mut self, other: &Self) {
        self.0.intersect_with(&other.0);
    }
}

impl<I: SparseIndex> From<FullAccess<I>> for Access<I> {
    fn from(value: FullAccess<I>) -> Self {
        let access = value
            .read
            .ones()
            .chain(value.write.ones())
            .collect::<FixedBitSet>();

        Access(access, Default::default())
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SystemAccessError {
    Component(usize),
    Resource(usize),
}

pub struct SystemAccess {
    resources: FullAccess<ResourceId>,
    components: FullAccess<ComponentId>,
    children: Vec<SystemAccess>,
}

impl SystemAccess {
    pub fn new() -> Self {
        Self {
            resources: FullAccess::new(),
            components: FullAccess::new(),
            children: Vec::new(),
        }
    }

    pub fn resources(&self) -> &FullAccess<ResourceId> {
        &self.resources
    }

    pub fn resources_mut(&mut self) -> &mut FullAccess<ResourceId> {
        &mut self.resources
    }

    pub fn components(&self) -> &FullAccess<ComponentId> {
        &self.components
    }

    pub fn components_mut(&mut self) -> &mut FullAccess<ComponentId> {
        &mut self.components
    }

    pub(crate) fn add_child(&mut self, access: SystemAccess) {
        self.children.push(access);
    }

    pub fn validate(&self) -> Result<(), SystemAccessError> {
        let _ = self
            .resources
            .validate()
            .map_err(SystemAccessError::Resource)?;

        self.components
            .validate()
            .map_err(SystemAccessError::Component)
    }

    pub fn flatten(mut self) -> Result<Vec<SystemAccess>, SystemAccessError> {
        let mut stack = std::mem::take(&mut self.children);
        let mut accesses = vec![self];

        while let Some(mut child) = stack.pop() {
            let children = std::mem::take(&mut child.children);
            for access in &accesses {
                if let Err(conflict) = access.resources.conflicts(&child.resources) {
                    return Err(SystemAccessError::Resource(conflict));
                }

                if let Err(conflict) = access.components.conflicts(&child.components) {
                    return Err(SystemAccessError::Component(conflict));
                }
            }

            accesses.push(child);
            stack.extend(children);
        }

        Ok(accesses)
    }
}

pub struct SystemMeta {
    pub id: SystemId,
    pub name: Option<SystemName>,
    /// Components that the system accesses.
    pub components: Access<ComponentId>,
    /// Resources that the system accesses.
    pub resources: Access<ResourceId>,
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
            components: Access::new(),
            resources: Access::new(),
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
        let mut access = SystemAccess::new();
        let state = (self.init)(world, &mut access);
        let access = match access.flatten() {
            Ok(access) => access,
            Err(error) => match error {
                SystemAccessError::Component(id) => {
                    let component = world
                        .components()
                        .get_by_id(ComponentId::from_usize(id))
                        .unwrap();

                    panic!(
                        "Invalid component access: {} for system: {:?}",
                        component.name(),
                        self.name
                    );
                }
                SystemAccessError::Resource(id) => {
                    let resource = world
                        .resources()
                        .get_meta(ResourceId::from_usize(id))
                        .unwrap();

                    panic!(
                        "Invalid resource access: {} for system: {:?}",
                        resource.name(),
                        self.name
                    );
                }
            },
        };

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
    pub access: Vec<SystemAccess>,
}

impl SystemNode {
    pub fn has_dependency(&self, other: &SystemNode) -> bool {
        if self.config.dependencies.contains(&other.config.id) {
            return true;
        }

        self.access.iter().any(|a| {
            other.access.iter().any(|b| {
                a.components.conflicts(&b.components).is_err()
                    || a.resources.conflicts(&b.resources).is_err()
            })
        })
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
pub type SystemInitFn = fn(&mut World, &mut SystemAccess) -> Box<dyn Any + Send + Sync>;
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
        let mut components = Access::new();
        let mut resources = Access::new();
        for access in value.access {
            components.merge(&Access::from(access.components));
            resources.merge(&Access::from(access.resources));
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
