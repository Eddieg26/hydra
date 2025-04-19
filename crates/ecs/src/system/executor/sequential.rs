use super::SystemExecutor;
use crate::{
    core::{DagValues, IndexDag},
    system::SystemCell,
};

pub struct SequentialExecutor {
    systems: Box<[SystemCell]>,
    order: Box<[usize]>,
}

impl SequentialExecutor {
    pub fn new(systems: IndexDag<SystemCell>) -> Self {
        let DagValues {
            nodes, topology, ..
        } = systems.into_values();

        Self {
            systems: nodes.into_boxed_slice(),
            order: topology.into_boxed_slice(),
        }
    }
}

impl SystemExecutor for SequentialExecutor {
    fn execute(&self, mut world: crate::world::WorldCell) {
        for index in &self.order {
            let system = &self.systems[*index];
            unsafe {
                system.cast_mut().run(world);
                system.cast_mut().apply(world.get_mut())
            };
        }
    }
}
