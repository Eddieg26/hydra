use crate::{
    Window,
    app::WindowApp,
    events::{
        AxisMotion, CursorEntered, CursorLeft, CursorMoved, DoubleTapGesture, DroppedFile,
        HoveredFile, HoveredFileCancelled, KeyEvent, ModifiersChanged, MouseInput, MouseScroll,
        PanGesture, PinchGesture, RotationGesture, ScaleFactorChanged, TouchpadPressure,
        WindowClosed, WindowCreated, WindowDestroyed, WindowFocused, WindowMoved, WindowOccluded,
        WindowResized,
    },
};
use ecs::{AppBuilder, app::Plugin};

pub struct WindowPlugin;

impl Plugin for WindowPlugin {
    fn setup(&mut self, app: &mut AppBuilder) {
        app.register_event::<WindowCreated>()
            .register_event::<WindowClosed>()
            .register_event::<AxisMotion>()
            .register_event::<WindowResized>()
            .register_event::<WindowMoved>()
            .register_event::<WindowDestroyed>()
            .register_event::<DroppedFile>()
            .register_event::<HoveredFile>()
            .register_event::<HoveredFileCancelled>()
            .register_event::<WindowFocused>()
            .register_event::<KeyEvent>()
            .register_event::<ModifiersChanged>()
            .register_event::<CursorMoved>()
            .register_event::<CursorEntered>()
            .register_event::<CursorLeft>()
            .register_event::<MouseScroll>()
            .register_event::<MouseInput>()
            .register_event::<PinchGesture>()
            .register_event::<PanGesture>()
            .register_event::<DoubleTapGesture>()
            .register_event::<RotationGesture>()
            .register_event::<TouchpadPressure>()
            .register_event::<ScaleFactorChanged>()
            .register_event::<WindowOccluded>()
            .register_resource::<Window>()
            .set_runner(WindowApp::runner);
    }
}
