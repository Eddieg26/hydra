use super::SystemCell;
use crate::{core::IndexDag, world::WorldCell};

pub mod parallel;
pub mod sequential;

pub use parallel::*;
pub use sequential::*;

pub trait SystemExecutor: 'static {
    fn execute(&self, world: WorldCell);
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum RunMode {
    Sequential,
    Parallel,
}

impl RunMode {
    pub fn create_executor(&self, systems: IndexDag<SystemCell>) -> Box<dyn SystemExecutor> {
        match self {
            RunMode::Sequential => Box::new(SequentialExecutor::new(systems)),
            RunMode::Parallel => Box::new(ParallelExecutor::new(systems)),
        }
    }
}
