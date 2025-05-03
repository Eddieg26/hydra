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

#[allow(unused_imports, dead_code)]
mod tests {
    use std::time::{Duration, Instant};
    use super::RunMode;
    use crate::{Phase, Resource, Schedule, World};

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Value(u32);
    impl Resource for Value {}

    impl std::ops::Deref for Value {
        type Target = u32;

        fn deref(&self) -> &Self::Target {
            &self.0
        }
    }

    impl std::ops::DerefMut for Value {
        fn deref_mut(&mut self) -> &mut Self::Target {
            &mut self.0
        }
    }

    pub struct Root;
    impl Phase for Root {}

    #[test]
    fn test_sequential() {
        let mut world = World::new();
        world.add_resource(Value(0));

        let mut schedule = Schedule::new(RunMode::Sequential);
        schedule.add_systems(Root, |value: &mut Value| {
            assert_eq!(**value, 0);
            value.0 = 1;
        });

        schedule.add_systems(Root, |value: &mut Value| {
            assert_eq!(**value, 1);
            value.0 = 2;
        });

        let systems = schedule.build(&mut world).unwrap();
        systems.run(Root, &mut world);

        assert_eq!(**world.resource::<Value>(), 2);
    }

    #[test]
    fn test_parallel() {
        let mut world = World::new();
        world.add_resource(Value(0));

        let mut schedule = Schedule::new(RunMode::Parallel);
        schedule.add_systems(Root, |_: &Value| {
            std::thread::sleep(Duration::from_secs(2));
        });

        schedule.add_systems(Root, |_: &Value| {
            std::thread::sleep(Duration::from_secs(2));
        });

        let systems = schedule.build(&mut world).unwrap();

        let time = Instant::now();
        systems.run(Root, &mut world);
        let duration = time.elapsed().as_secs();

        assert!(duration < 3 && duration > 1);
    }

    #[test]
    fn test_exclusive() {
        let mut world = World::new();
        world.add_resource(Value(0));

        let mut schedule = Schedule::new(RunMode::Parallel);
        schedule.add_systems(Root, |value: &mut Value| {
            assert_eq!(**value, 0);
            value.0 += 1;
        });

        schedule.add_systems(Root, |world: &World| unsafe {
            let mut world = world.cell();
            let value = world.get_mut().resource_mut::<Value>();
            assert_eq!(**value, 1);
            value.0 += 1;
        });

        schedule.add_systems(Root, |value: &mut Value| {
            assert_eq!(**value, 2);
            value.0 += 1;
        });

        let systems = schedule.build(&mut world).unwrap();
        systems.run(Root, &mut world);

        assert_eq!(**world.resource::<Value>(), 3);
    }
}
