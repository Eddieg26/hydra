use crate::{
    ActionRegistry, Actions, Buttons, DoubleTapGesture, GamepadEvent, Gamepads, InputReceiver, Key,
    KeyCode, KeyboardInput, MouseButton, MouseInput, MouseScroll, PinchGesture, RotationGesture,
    TouchInput, TouchpadPressure,
};
use ecs::{AppBuilder, Plugin, Start, app::PreUpdate};

pub struct InputPlugin;

impl Plugin for InputPlugin {
    fn setup(&mut self, app: &mut ecs::AppBuilder) {
        app.add_resource(ActionRegistry::new());
    }
}

pub struct MousePlugin;
impl Plugin for MousePlugin {
    fn setup(&mut self, app: &mut AppBuilder) {
        app.add_plugins(InputPlugin)
            .register_event::<MouseInput>()
            .register_event::<MouseScroll>()
            .add_resource(Buttons::<MouseButton>::default())
            .add_systems(Start, MouseInput::process_events);
    }
}

pub struct KeyboardPlugin;
impl Plugin for KeyboardPlugin {
    fn setup(&mut self, app: &mut AppBuilder) {
        app.add_plugins(InputPlugin)
            .register_event::<KeyboardInput>()
            .add_resource(Buttons::<KeyCode>::default())
            .add_resource(Buttons::<Key>::default())
            .add_systems(Start, KeyboardInput::process_events);
    }
}

pub struct GamepadPlugin;
impl Plugin for GamepadPlugin {
    fn setup(&mut self, app: &mut AppBuilder) {
        app.add_plugins(InputPlugin)
            .add_resource(Gamepads::new())
            .add_systems(Start, Gamepads::process_events)
            .register_event::<GamepadEvent>();
    }
}

pub struct TouchPlugin;
impl Plugin for TouchPlugin {
    fn setup(&mut self, app: &mut AppBuilder) {
        app.add_plugins(InputPlugin)
            .register_event::<PinchGesture>()
            .register_event::<DoubleTapGesture>()
            .register_event::<RotationGesture>()
            .register_event::<TouchpadPressure>()
            .register_event::<TouchInput>();
    }
}

pub struct InputReceiverPlugin<R: InputReceiver>(Actions, std::marker::PhantomData<R>);
impl<R: InputReceiver> Plugin for InputReceiverPlugin<R> {
    fn setup(&mut self, app: &mut AppBuilder) {
        app.add_plugins(InputPlugin)
            .register::<R>()
            .resource_mut::<ActionRegistry>()
            .register::<R>(std::mem::take(&mut self.0));
        app.add_systems(PreUpdate, ActionRegistry::send_action_events::<R>);
    }
}

pub trait InputAppExt {
    fn add_input_receiver<R: InputReceiver>(&mut self, actions: Actions) -> &mut Self;
}

impl InputAppExt for AppBuilder {
    fn add_input_receiver<R: InputReceiver>(&mut self, actions: Actions) -> &mut Self {
        self.add_plugins(InputReceiverPlugin::<R>(actions, Default::default()))
    }
}
