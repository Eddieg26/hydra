use super::{
    IntoSystemConfigs, SystemCell, SystemConfig,
    executor::{RunMode, SystemExecutor},
};
use crate::{
    core::{ImmutableIndexDag, IndexDag},
    ext::{self},
    world::{World, WorldCell},
};
use std::collections::HashMap;

pub struct PhaseContext<'a> {
    world: WorldCell<'a>,
    executor: &'a dyn SystemExecutor,
}

impl<'a> PhaseContext<'a> {
    pub(crate) fn new(world: WorldCell<'a>, executor: &'a dyn SystemExecutor) -> Self {
        Self { world, executor }
    }

    pub unsafe fn world(&self) -> WorldCell {
        self.world
    }

    pub fn execute(&self) {
        self.executor.execute(self.world);
    }
}

pub trait Phase: 'static {
    fn run(&self, ctx: PhaseContext) {
        ctx.execute();
    }

    fn name(&self) -> &'static str {
        ext::short_type_name::<Self>()
    }
}

pub struct PhaseConfig {
    phase: Box<dyn Phase>,
    configs: Vec<SystemConfig>,
    parent: Option<usize>,
}

impl PhaseConfig {
    pub fn new(phase: impl Phase) -> Self {
        Self {
            phase: Box::new(phase),
            configs: vec![],
            parent: None,
        }
    }

    pub fn add_systems<M>(&mut self, systems: impl IntoSystemConfigs<M>) {
        self.configs.extend(systems.configs().flatten());
    }

    pub fn set_parent(&mut self, index: usize) {
        self.parent = Some(index)
    }

    pub fn build(self, world: &mut World, mode: RunMode) -> PhaseNode {
        let mut systems = IndexDag::new();
        for config in self.configs {
            systems.add_node(config.into_node(world));
        }

        for index in (0..systems.nodes().len()).rev() {
            for dep_index in (0..systems.nodes().len()).take(index) {
                if systems.nodes()[index].has_dependency(&systems.nodes()[dep_index]) {
                    systems.add_dependency(dep_index, index);
                }
            }
        }

        if let Err(error) = systems.build() {
            let systems = error
                .0
                .iter()
                .map(|i| {
                    systems.nodes()[*i]
                        .config
                        .name()
                        .cloned()
                        .unwrap_or("anonymous".into())
                })
                .collect::<Vec<_>>();

            let phase = self.phase.name();

            panic!(
                "Cyclic dependency detected in phase {}: {:?}",
                phase, systems
            );
        }

        let executor = mode.create_executor(systems.map(SystemCell::from));

        PhaseNode {
            phase: self.phase,
            executor,
        }
    }
}

pub struct PhaseNode {
    phase: Box<dyn Phase>,
    executor: Box<dyn SystemExecutor>,
}

impl PhaseNode {
    pub fn run(&self, world: WorldCell) {
        let ctx = PhaseContext::new(world, self.executor.as_ref());
        self.phase.run(ctx);
    }
}

pub struct Schedule {
    mode: RunMode,
    phases: IndexDag<PhaseConfig>,
    hierarchy: IndexDag<usize>,
    map: HashMap<&'static str, usize>,
}

impl Schedule {
    pub fn new(mode: RunMode) -> Self {
        Self {
            mode,
            phases: IndexDag::new(),
            hierarchy: IndexDag::new(),
            map: HashMap::new(),
        }
    }

    pub fn mode(&self) -> RunMode {
        self.mode
    }

    pub fn set_mode(&mut self, mode: RunMode) {
        self.mode = mode;
    }

    pub fn add_phase(&mut self, phase: impl Phase) -> usize {
        match self.map.get(phase.name()).copied() {
            Some(index) => index,
            None => {
                let config = PhaseConfig::new(phase);
                let index = self.phases.add_node(config);
                self.map
                    .insert(self.phases.nodes()[index].phase.name(), index);
                self.hierarchy.add_node(index);
                index
            }
        }
    }

    pub fn add_sub_phase(&mut self, main: impl Phase, sub: impl Phase) {
        let main_index = self.add_phase(main);
        let sub_index = self.add_phase(sub);

        self.hierarchy.add_dependency(main_index, sub_index);
        self.phases.nodes_mut()[sub_index].parent = Some(main_index);
    }

    pub fn run_before(&mut self, phase: impl Phase, target: impl Phase) {
        let index = self.add_phase(phase);
        let target_index = self.add_phase(target);

        self.phases.add_dependency(index, target_index);

        if let Some(old_parent) = self.phases.nodes()[index].parent {
            self.hierarchy.remove_dependency(old_parent, index);
        }

        self.phases.nodes_mut()[index].parent = self.phases.nodes()[target_index].parent;
        if let Some(parent) = self.phases.nodes()[index].parent {
            self.hierarchy.add_dependency(parent, index);
        }
    }

    pub fn run_after(&mut self, phase: impl Phase, target: impl Phase) {
        self.run_before(target, phase);
    }

    pub fn add_systems<M>(&mut self, phase: impl Phase, systems: impl IntoSystemConfigs<M>) {
        let index = self.add_phase(phase);
        self.phases.nodes_mut()[index].add_systems(systems);
    }

    pub fn build(self, world: &mut World) -> Result<Systems, ScheduleBuildError> {
        let mode = self.mode;
        let mut hierarchy = self.hierarchy;
        let mut phases = self.phases;

        if let Err(error) = hierarchy.build() {
            let names = error
                .0
                .iter()
                .map(|index| phases.nodes()[*index].phase.name())
                .collect();
            return Err(ScheduleBuildError::CyclicHierarchy(names));
        }

        if let Err(error) = phases.build() {
            let names = error
                .0
                .iter()
                .map(|index| phases.nodes()[*index].phase.name())
                .collect();
            return Err(ScheduleBuildError::CyclicDependency(names));
        }

        let mut hierarchy = HashMap::new();
        for index in phases.topology() {
            if let Some(parent) = phases.nodes()[*index].parent {
                hierarchy.entry(parent).or_insert(vec![]).push(*index);
            }
        }

        let phases = phases.map(|config| config.build(world, mode));

        Ok(Systems {
            mode,
            phases: phases.into_immutable(),
            hierarchy,
            map: self.map,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScheduleBuildError {
    CyclicDependency(Vec<&'static str>),
    CyclicHierarchy(Vec<&'static str>),
}

impl std::fmt::Display for ScheduleBuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ScheduleBuildError::CyclicDependency(names) => {
                write!(f, "Cyclic dependency detected: {:?}", names)
            }
            ScheduleBuildError::CyclicHierarchy(names) => {
                write!(f, "Cyclic hierarchy detected: {:?}", names)
            }
        }
    }
}

pub struct Systems {
    mode: RunMode,
    phases: ImmutableIndexDag<PhaseNode>,
    hierarchy: HashMap<usize, Vec<usize>>,
    map: HashMap<&'static str, usize>,
}

impl Systems {
    pub fn mode(&self) -> RunMode {
        self.mode
    }

    pub fn run(&self, phase: impl Phase, world: &mut World) {
        if let Some(index) = self.map.get(phase.name()).copied() {
            let world = unsafe { WorldCell::new_mut(world) };

            let mut stack = vec![index];
            while let Some(index) = stack.pop() {
                self.phases.nodes()[index].run(world);
                if let Some(children) = self.hierarchy.get(&index) {
                    for child in children.iter().rev() {
                        stack.insert(0, *child);
                    }
                }
            }
        }
    }
}

#[allow(unused_imports, dead_code)]
mod tests {
    use crate::{
        system::{
            executor::RunMode,
            schedule::{Schedule, ScheduleBuildError},
        },
        world::World,
    };

    #[derive(Clone, Copy, PartialEq, Eq)]
    struct TestPhase(&'static str);

    impl super::Phase for TestPhase {
        fn name(&self) -> &'static str {
            self.0
        }
    }

    #[test]
    fn test_phase_ordering() {
        let mut schedule = Schedule::new(RunMode::Sequential);
        let phase1 = TestPhase("Phase1");
        let phase2 = TestPhase("Phase2");
        let phase3 = TestPhase("Phase3");

        schedule.run_before(phase2, phase3); // Phase2 runs before Phase3
        schedule.run_after(phase1, phase3); // Phase1 runs after Phase3

        let mut world = World::new();
        let systems = schedule.build(&mut world).unwrap();

        let topology: Vec<_> = systems
            .phases
            .topology()
            .iter()
            .map(|&i| systems.phases.nodes()[i].phase.name())
            .collect();
        assert_eq!(topology, vec!["Phase2", "Phase3", "Phase1"]);
    }

    #[test]
    fn test_hierarchy() {
        let mut schedule = Schedule::new(RunMode::Sequential);
        let main_phase = TestPhase("MainPhase");
        let sub_phase1 = TestPhase("SubPhase1");
        let sub_phase2 = TestPhase("SubPhase2");

        schedule.add_sub_phase(main_phase, sub_phase1);
        schedule.add_sub_phase(main_phase, sub_phase2);

        let mut world = World::new();
        let systems = schedule.build(&mut world).unwrap();

        let main_index = systems.map["MainPhase"];
        let sub_indices = systems.hierarchy.get(&main_index).unwrap();
        let sub_names: Vec<_> = sub_indices
            .iter()
            .map(|&i| systems.phases.nodes()[i].phase.name())
            .collect();

        assert!(sub_names.contains(&"SubPhase1"));
        assert!(sub_names.contains(&"SubPhase2"));
    }

    #[test]
    fn test_cyclic_dependency_error() {
        let mut schedule = Schedule::new(RunMode::Sequential);
        let phase1 = TestPhase("Phase1");
        let phase2 = TestPhase("Phase2");
        let phase3 = TestPhase("Phase3");

        schedule.run_before(phase2, phase3); // Phase2 runs before Phase3
        schedule.run_before(phase3, phase1); // Phase3 runs before Phase1
        schedule.run_before(phase1, phase2); // Phase1 runs before Phase2 (creates a cycle)

        let mut world = World::new();
        let result = schedule.build(&mut world);

        assert!(result.is_err());
        if let Err(ScheduleBuildError::CyclicDependency(names)) = result {
            assert!(names.contains(&"Phase1"));
            assert!(names.contains(&"Phase2"));
            assert!(names.contains(&"Phase3"));
        } else {
            panic!("Expected a cyclic dependency error");
        }
    }

    #[test]
    fn test_cyclic_hierarchy_error() {
        let mut schedule = Schedule::new(RunMode::Sequential);
        let main_phase = TestPhase("MainPhase");
        let sub_phase = TestPhase("SubPhase");

        schedule.add_sub_phase(main_phase, sub_phase);
        schedule.add_sub_phase(sub_phase, main_phase); // Creates a cyclic hierarchy

        let mut world = World::new();
        let result = schedule.build(&mut world);

        assert!(result.is_err());
        if let Err(ScheduleBuildError::CyclicHierarchy(names)) = result {
            assert!(names.contains(&"MainPhase"));
            assert!(names.contains(&"SubPhase"));
        } else {
            panic!("Expected a cyclic hierarchy error");
        }
    }
}
