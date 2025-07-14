use super::SystemExecutor;
use crate::{
    core::{DagValues, IndexDag},
    system::SystemCell,
};
use fixedbitset::FixedBitSet;

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
        let mut set = FixedBitSet::with_capacity(self.systems.len());
        for index in &self.order {
            let system = &self.systems[*index];
            unsafe {
                let ran = system.cast_mut().run(world);
                set.set(*index, ran);
            };
        }

        for index in set.ones() {
            unsafe {
                let system = self.systems[index].cast_mut();
                system.update(world.get_mut())
            };
        }
    }
}
