use crate::{
    AppBuilder, Despawned, End, Extract, Init, Plugin, Resource, Run, Shutdown, Start, Update,
    app::{PostUpdate, PreUpdate, time::TimePlugin},
    core::task::TaskPoolSettings,
};
use derive_ecs::PluginKit;

#[derive(Default)]
pub struct TaskPoolPlugin;
impl Plugin for TaskPoolPlugin {
    fn setup(&mut self, app: &mut AppBuilder) {
        app.add_resource(TaskPoolSettings::default());
    }

    fn finish(&mut self, app: &mut AppBuilder) {
        let settings = app
            .remove_resource::<TaskPoolSettings>()
            .unwrap_or_default();
        settings.init_task_pools();
    }
}

#[derive(Default)]
pub struct DefaultPhases<const MAIN: bool = true>;
impl<const MAIN: bool> Plugin for DefaultPhases<MAIN> {
    fn setup(&mut self, app: &mut AppBuilder) {
        if MAIN {
            app.add_phase(Init);
            app.add_phase(Run);
            app.add_sub_phase(Run, Start);
            app.add_sub_phase(Run, PreUpdate);
            app.add_sub_phase(Run, Update);
            app.add_sub_phase(Run, PostUpdate);
            app.add_sub_phase(Run, End);
            app.add_phase(Shutdown);
        } else {
            app.add_phase(Init);
            app.add_phase(Run);
            app.add_phase(Extract);
            app.add_phase(Shutdown);
        }
    }
}

#[derive(Default)]
pub struct DefaultEvents;
impl Plugin for DefaultEvents {
    fn setup(&mut self, app: &mut AppBuilder) {
        app.register_event::<Despawned>();
    }
}

impl Resource for TaskPoolSettings {}

#[derive(Default, PluginKit)]
pub struct DefaultPlugins {
    task_pool: TaskPoolPlugin,
    phases: DefaultPhases,
    events: DefaultEvents,
    time: TimePlugin,
}
