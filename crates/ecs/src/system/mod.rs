use crate::{
    WorldAccess,
    core::Frame,
    world::{World, cell::WorldCell},
};
use fixedbitset::FixedBitSet;
use std::{any::Any, borrow::Cow, cell::UnsafeCell};

pub mod arg;
pub mod commands;
pub mod config;
pub mod executor;
pub mod query;
pub mod schedule;

pub use arg::*;
pub use commands::*;
pub use config::*;
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

pub type SystemState = Box<dyn Any + Send + Sync>;
pub type SystemInit = fn(&mut World, &mut WorldAccess) -> Box<dyn Any + Send + Sync>;
pub type SystemRun =
    Box<dyn Fn(&mut Box<dyn Any + Send + Sync>, WorldCell, &SystemMeta) + Send + Sync>;
pub type SystemUpdate = fn(&mut Box<dyn Any + Send + Sync>, &mut World);
pub type SystemCondition = fn(&World, &SystemMeta) -> bool;

pub struct System {
    meta: SystemMeta,
    state: SystemState,
    run: SystemRun,
    update: SystemUpdate,
    condition: SystemCondition,
}

impl System {
    pub fn new(
        meta: SystemMeta,
        state: SystemState,
        run: SystemRun,
        update: SystemUpdate,
        condition: SystemCondition,
    ) -> Self {
        Self {
            meta,
            state,
            run,
            update,
            condition,
        }
    }

    pub fn run(&mut self, world: WorldCell) -> bool {
        if self.should_run(unsafe { world.get() }) {
            (self.run)(&mut self.state, world, &self.meta);
            self.meta.frame = unsafe { world.get().frame() };
            true
        } else {
            false
        }
    }

    pub fn update(&mut self, world: &mut World) {
        (self.update)(&mut self.state, world);
    }

    pub fn should_run(&self, world: &World) -> bool {
        (self.condition)(world, &self.meta)
    }
}

pub struct SystemCell(UnsafeCell<System>);

impl From<System> for SystemCell {
    fn from(system: System) -> Self {
        Self(UnsafeCell::new(system))
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
