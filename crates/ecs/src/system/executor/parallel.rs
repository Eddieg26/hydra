use super::SystemExecutor;
use crate::{
    core::{
        ImmutableIndexDag, IndexDag,
        task::{self, Scope, TaskPool, scope},
    },
    system::SystemCell,
    world::WorldCell,
};
use fixedbitset::FixedBitSet;
use std::sync::{Arc, Mutex, MutexGuard};

pub struct ParallelExecutor {
    state: Arc<Mutex<ExecutionState>>,
    systems: ImmutableIndexDag<SystemCell>,
    initial_systems: FixedBitSet,
}

impl ParallelExecutor {
    pub fn new(systems: IndexDag<SystemCell>) -> Self {
        let systems = systems.into_immutable();

        let mut initial_systems = FixedBitSet::with_capacity(systems.len());
        for (index, deps) in systems.dependencies().iter().enumerate() {
            initial_systems.set(index, *deps == 0);
        }

        let state = ExecutionState {
            dependencies: systems.dependencies().to_vec(),
            queue: initial_systems.clone(),
            completed: FixedBitSet::with_capacity(systems.len()),
            skipped: FixedBitSet::with_capacity(systems.len()),
            running: 0,
        };

        if !scope::is_initialized() {
            scope::init(TaskPool::builder().build());
        }

        Self {
            state: Arc::new(Mutex::new(state)),
            systems,
            initial_systems,
        }
    }

    fn reset(&self, mut state: MutexGuard<'_, ExecutionState>) {
        state.running = 0;
        state.completed.clear();
        state.skipped.clear();
        state.queue = self.initial_systems.clone();
        state.dependencies = self.systems.dependencies().to_vec();
    }
}

impl SystemExecutor for ParallelExecutor {
    fn execute(&self, mut world: WorldCell) {
        task::scoped(|scope| {
            let ctx = ExecutionContext::new(world, &self.systems, scope, self.state.clone());
            ctx.execute();
        });

        let state = self.state.lock().unwrap();
        for index in self
            .systems
            .topology()
            .iter()
            .filter_map(|i| (!state.skipped[*i]).then_some(i))
        {
            unsafe {
                self.systems.nodes()[*index]
                    .cast_mut()
                    .update(world.get_mut())
            };
        }

        self.reset(state);
    }
}

pub struct ExecutionState {
    dependencies: Vec<usize>,
    queue: FixedBitSet,
    completed: FixedBitSet,
    skipped: FixedBitSet,
    running: usize,
}

impl Default for ExecutionState {
    fn default() -> Self {
        Self {
            dependencies: Default::default(),
            queue: Default::default(),
            completed: Default::default(),
            skipped: Default::default(),
            running: 0,
        }
    }
}

pub enum ExecutionResult {
    Run(usize),
    Done,
}

pub struct ExecutionContext<'scope, 'env: 'scope> {
    world: WorldCell<'scope>,
    systems: &'scope ImmutableIndexDag<SystemCell>,
    scope: Arc<Scope<'scope, 'env>>,
    state: Arc<Mutex<ExecutionState>>,
}

impl<'scope, 'env: 'scope> ExecutionContext<'scope, 'env> {
    pub fn new(
        world: WorldCell<'scope>,
        systems: &'env ImmutableIndexDag<SystemCell>,
        scope: Scope<'scope, 'env>,
        state: Arc<Mutex<ExecutionState>>,
    ) -> Self {
        Self {
            world,
            systems,
            scope: Arc::new(scope),
            state,
        }
    }

    fn scoped(&self) -> Self {
        let world = self.world;
        let systems = self.systems;
        let scope = self.scope.clone();
        let state = self.state.clone();

        Self {
            world,
            systems,
            scope,
            state,
        }
    }

    fn spawn(&self, index: usize) {
        let scoped = self.scoped();
        self.scope.spawn(async move { scoped.run_system(index) });
    }

    fn spawn_non_send(&self, index: usize) {
        let scoped = self.scoped();
        self.scope
            .spawn_local(async move { scoped.run_system(index) });
    }

    fn execute(&self) {
        let state = self.state.lock().unwrap();
        self.spawn_systems(state);
    }

    fn spawn_systems(&self, mut state: MutexGuard<'_, ExecutionState>) {
        if state.completed.is_full() {
            return;
        }

        for index in state.queue.clone().into_ones() {
            let system = self.systems.nodes()[index].get();

            if system.meta.send {
                state.queue.set(index, false);
                state.running += 1;
                self.spawn(index);
            } else if !system.meta.exclusive {
                state.queue.set(index, false);
                state.running += 1;
                self.spawn_non_send(index);
            } else {
                if state.running == 0 {
                    self.spawn_non_send(index);
                }

                break;
            }
        }
    }

    fn run_system(&self, index: usize) {
        let skipped = unsafe { self.systems.nodes()[index].cast_mut().run(self.world) };
        self.system_done(index, skipped);
    }

    fn system_done(&self, index: usize, skipped: bool) {
        let mut state = self.state.lock().unwrap();

        state.running -= 1;
        state.completed.set(index, true);
        state.skipped.set(index, skipped);

        for dependent in self.systems.dependents()[index].ones() {
            state.dependencies[dependent] -= 1;
            if state.dependencies[dependent] == 0 {
                state.queue.set(dependent, true);
            }
        }

        self.spawn_systems(state);
    }
}
