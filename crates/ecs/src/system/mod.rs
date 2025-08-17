use crate::{
    WorldAccess,
    core::Frame,
    world::{World, cell::WorldCell},
};
use fixedbitset::FixedBitSet;
use std::{any::Any, cell::UnsafeCell};

pub mod arg;
pub mod config;
pub mod executor;
pub mod query;
pub mod schedule;
pub mod set;

pub use arg::*;
pub use config::*;
pub use executor::*;
pub use query::*;
pub use schedule::*;
pub use set::*;

pub type SystemName = &'static str;

pub struct SystemMeta {
    pub id: SystemId,
    pub name: SystemName,
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
            id: SystemType::<()>::new().identify(),
            name: "default",
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
            true
        } else {
            false
        }
    }

    pub fn update(&mut self, world: &mut World) {
        self.meta.frame = world.frame();
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
