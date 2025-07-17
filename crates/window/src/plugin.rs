use crate::{
    Window,
    app::WindowApp,
    events::{
        AxisMotion, CursorEntered, CursorLeft, CursorMoved, DroppedFile, HoveredFile,
        HoveredFileCancelled, ModifiersChanged, ScaleFactorChanged, WindowClosed, WindowCreated,
        WindowDestroyed, WindowFocused, WindowMoved, WindowOccluded, WindowResized,
    },
};
use ecs::{AppBuilder, app::Plugin};
use input::plugin::{KeyboardPlugin, MousePlugin, TouchPlugin};

pub struct WindowPlugin;

impl Plugin for WindowPlugin {
    fn setup(&mut self, app: &mut AppBuilder) {
        app.add_plugins((KeyboardPlugin, MousePlugin, TouchPlugin))
            .register_event::<WindowCreated>()
            .register_event::<WindowClosed>()
            .register_event::<AxisMotion>()
            .register_event::<WindowResized>()
            .register_event::<WindowMoved>()
            .register_event::<WindowDestroyed>()
            .register_event::<DroppedFile>()
            .register_event::<HoveredFile>()
            .register_event::<HoveredFileCancelled>()
            .register_event::<WindowFocused>()
            .register_event::<ModifiersChanged>()
            .register_event::<CursorMoved>()
            .register_event::<CursorEntered>()
            .register_event::<CursorLeft>()
            .register_event::<ScaleFactorChanged>()
            .register_event::<WindowOccluded>()
            .register_resource::<Window>()
            .set_runner(WindowApp::runner);
    }
}
