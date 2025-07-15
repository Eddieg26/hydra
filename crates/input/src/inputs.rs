use std::collections::HashMap;
use ecs::Component;
use crate::gamepad::GamepadButton;

pub enum InputBinding {
    Gamepad {
        button: GamepadButton,
        state: ButtonState,
        repeat: bool,
    }
}

#[derive(Component)]
pub struct Inputs {
    inputs: HashMap<String, InputBinding>,
}
