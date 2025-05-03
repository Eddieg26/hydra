use crate::{
    system::{
        IntoSystemConfigs,
        executor::RunMode,
        schedule::{Phase, Schedule, Systems},
    },
    world::{Component, Resource, World},
};

pub struct AppBuilder {
    world: World,
    schedule: Schedule,
}

impl AppBuilder {
    pub fn new() -> Self {
        Self {
            world: World::new(),
            schedule: Schedule::new(RunMode::Sequential),
        }
    }

    pub fn world(&self) -> &World {
        &self.world
    }

    pub fn world_mut(&mut self) -> &mut World {
        &mut self.world
    }

    pub fn schedule(&self) -> &Schedule {
        &self.schedule
    }

    pub fn schedule_mut(&mut self) -> &mut Schedule {
        &mut self.schedule
    }

    pub fn register<C: Component>(&mut self) -> &mut Self {
        self.world.register::<C>();
        self
    }

    pub fn add_resource<R: Resource + Send>(&mut self, resource: R) -> &mut Self {
        self.world.add_resource(resource);
        self
    }

    pub fn add_non_send_resource<R: Resource>(&mut self, resource: R) -> &mut Self {
        self.world.add_non_send_resource(resource);
        self
    }

    pub fn add_phase(&mut self, phase: impl Phase) -> &mut Self {
        self.schedule.add_phase(phase);
        self
    }

    pub fn add_sub_phase(&mut self, main: impl Phase, sub: impl Phase) -> &mut Self {
        self.schedule.add_sub_phase(main, sub);
        self
    }

    pub fn add_phase_before(&mut self, phase: impl Phase, before: impl Phase) -> &mut Self {
        self.schedule.run_before(phase, before);
        self
    }

    pub fn add_phase_after(&mut self, phase: impl Phase, after: impl Phase) -> &mut Self {
        self.schedule.run_after(phase, after);
        self
    }

    pub fn add_systems<M>(
        &mut self,
        phase: impl Phase,
        systems: impl IntoSystemConfigs<M>,
    ) -> &mut Self {
        self.schedule.add_systems(phase, systems);
        self
    }

    pub fn build(&mut self) -> App {
        let mut app = std::mem::take(self);
        let systems = app.schedule.build(&mut app.world).unwrap();

        App {
            world: app.world,
            systems,
        }
    }
}

impl Default for AppBuilder {
    fn default() -> Self {
        Self::new()
    }
}

pub struct App {
    world: World,
    systems: Systems,
}

impl App {
    pub fn new() -> AppBuilder {
        AppBuilder::new()
    }

    pub fn world(&self) -> &World {
        &self.world
    }

    pub fn systems(&self) -> &Systems {
        &self.systems
    }

    pub fn is_send(&self) -> bool {
        self.world.resources().is_send()
    }

    pub fn run(&mut self, phase: impl Phase) -> &mut Self {
        self.systems.run(phase, &mut self.world);
        self.world.update();
        self
    }
}
