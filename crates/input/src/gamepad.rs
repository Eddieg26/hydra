use crate::{ButtonState, analog::Analogs, button::Buttons};
use ecs::{Event, EventReader, Resource};
use math::Vec2;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GamepadId(pub(crate) usize);
impl From<usize> for GamepadId {
    fn from(value: usize) -> Self {
        Self(value)
    }
}

#[derive(Debug)]
pub struct Gamepad {
    /// The status of the gamepad.
    pub(crate) status: GamepadStatus,
    /// The USB vendor id assigned by USB-IF.
    pub(crate) vendor_id: Option<u16>,
    /// The USB product id assigned by the vendor.
    pub(crate) product_id: Option<u16>,
    /// The state of the gamepad buttons.
    pub(crate) buttons: Buttons<GamepadButton>,
    /// The state of the gamepad axes.
    pub(crate) analogs: Analogs<GamepadInput>,
}

impl Gamepad {
    pub fn status(&self) -> GamepadStatus {
        self.status
    }

    pub fn vendor_id(&self) -> Option<u16> {
        self.vendor_id
    }

    pub fn product_id(&self) -> Option<u16> {
        self.product_id
    }

    pub fn buttons(&self) -> &Buttons<GamepadButton> {
        &self.buttons
    }

    pub fn buttons_mut(&mut self) -> &mut Buttons<GamepadButton> {
        &mut self.buttons
    }

    pub fn analogs(&self) -> &Analogs<GamepadInput> {
        &self.analogs
    }

    pub fn analogs_mut(&mut self) -> &mut Analogs<GamepadInput> {
        &mut self.analogs
    }

    pub fn button(&self, button: &GamepadButton) -> Option<ButtonState> {
        self.buttons.get(button)
    }

    pub fn pressed(&self, button: GamepadButton) -> bool {
        self.buttons.get(&button) == Some(ButtonState::Pressed)
    }

    pub fn released(&self, button: GamepadButton) -> bool {
        self.buttons.get(&button) == Some(ButtonState::Released)
    }

    pub fn down(&self, button: GamepadButton) -> bool {
        self.buttons.down(&button)
    }

    pub fn analog(&self, input: impl Into<GamepadInput>) -> Option<f32> {
        self.analogs.get(&input.into())
    }

    pub fn analog_unclamped(&self, input: impl Into<GamepadInput>) -> Option<f32> {
        self.analogs.get_unclamed(&input.into())
    }

    pub fn right(&self) -> Vec2 {
        let x = self.analog(GamepadAxis::RightStickX).unwrap_or(0.0);
        let y = self.analog(GamepadAxis::RightStickY).unwrap_or(0.0);
        Vec2::new(x, y)
    }

    pub fn left(&self) -> Vec2 {
        let x = self.analog(GamepadAxis::LeftStickX).unwrap_or(0.0);
        let y = self.analog(GamepadAxis::LeftStickY).unwrap_or(0.0);
        Vec2::new(x, y)
    }

    pub fn dpad(&self) -> Vec2 {
        let x = self.analog(GamepadAxis::DPadX).unwrap_or(0.0);
        let y = self.analog(GamepadAxis::DPadY).unwrap_or(0.0);
        Vec2::new(x, y)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GamepadStatus {
    Connected,
    Disconnected,
}

#[derive(Debug, Clone, Event)]
pub enum GamepadEvent {
    /// A gamepad has been connected.
    Connected {
        /// The id of the gamepad.
        id: GamepadId,
        /// The name of the gamepad.
        /// This is typically the name of the device as reported by the system.
        /// For example, "Xbox Wireless Controller".
        name: String,

        /// The USB vendor id assigned by USB-IF.
        vendor_id: Option<u16>,

        /// The USB product id assigned by the vendor.
        product_id: Option<u16>,
    },

    /// A gamepad has been disconnected.
    Disconnected(GamepadId),

    /// A gamepad input event.
    /// This event is emitted when a gamepad button is pressed, released, or an axis value changes.
    Input {
        /// The id of the gamepad.
        id: GamepadId,
        /// The type of the input event.
        value: GamepadInputEvent,
    },
}

#[derive(Resource, Debug)]
pub struct Gamepads {
    gamepads: HashMap<GamepadId, Gamepad>,
}

impl Gamepads {
    pub fn new() -> Self {
        Self {
            gamepads: HashMap::new(),
        }
    }

    pub fn connect(&mut self, id: GamepadId, gamepad: Gamepad) {
        self.gamepads
            .entry(id)
            .insert_entry(gamepad)
            .get_mut()
            .status = GamepadStatus::Connected;
    }

    pub fn disconnect(&mut self, id: &GamepadId) {
        if let Some(pad) = self.gamepads.get_mut(id) {
            pad.status = GamepadStatus::Disconnected;
        }
    }

    pub fn get(&self, id: &GamepadId) -> Option<&Gamepad> {
        self.gamepads.get(id)
    }

    pub fn get_mut(&mut self, id: &GamepadId) -> Option<&mut Gamepad> {
        self.gamepads.get_mut(id)
    }

    pub fn connected(&self) -> impl Iterator<Item = (&GamepadId, &Gamepad)> {
        self.gamepads
            .iter()
            .filter(|(_, gamepad)| gamepad.status == GamepadStatus::Connected)
    }

    pub fn disconnected(&self) -> impl Iterator<Item = (&GamepadId, &Gamepad)> {
        self.gamepads
            .iter()
            .filter(|(_, gamepad)| gamepad.status == GamepadStatus::Disconnected)
    }

    pub(crate) fn process_events(gamepads: &mut Gamepads, events: EventReader<GamepadEvent>) {
        for event in events {
            match event {
                GamepadEvent::Connected {
                    id,
                    vendor_id,
                    product_id,
                    ..
                } => {
                    let gamepad = Gamepad {
                        status: GamepadStatus::Connected,
                        vendor_id: *vendor_id,
                        product_id: *product_id,
                        buttons: Buttons::new(),
                        analogs: Analogs::new(),
                    };

                    gamepads.connect(*id, gamepad);
                }
                GamepadEvent::Disconnected(id) => gamepads.disconnect(id),
                GamepadEvent::Input { id, value } => match gamepads.get_mut(id) {
                    Some(gamepad) => match value {
                        GamepadInputEvent::ButtonPressed(button) => {
                            gamepad.buttons.set(*button, ButtonState::Pressed)
                        }
                        GamepadInputEvent::ButtonRepeated(button) => {
                            gamepad.buttons.set(*button, ButtonState::Pressed)
                        }
                        GamepadInputEvent::ButtonReleased(button) => {
                            gamepad.buttons.set(*button, ButtonState::Released)
                        }
                        GamepadInputEvent::ButtonChanged { button, value } => {
                            gamepad.analogs.set((*button).into(), *value)
                        }
                        GamepadInputEvent::AxisChanged { axis, value } => {
                            gamepad.analogs.set((*axis).into(), *value)
                        }
                    },
                    None => {}
                },
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GamepadAxis {
    LeftStickX,
    LeftStickY,
    LeftZ,
    RightStickX,
    RightStickY,
    RightZ,
    DPadX,
    DPadY,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GamepadButton {
    // Action Pad
    South,
    East,
    North,
    West,
    C,
    Z,
    // Triggers
    LeftTrigger,
    LeftTrigger2,
    RightTrigger,
    RightTrigger2,
    // Menu Pad
    Select,
    Start,
    Mode,
    // Sticks
    LeftThumb,
    RightThumb,
    // D-Pad
    DPadUp,
    DPadDown,
    DPadLeft,
    DPadRight,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GamepadInput {
    Axis(GamepadAxis),
    Button(GamepadButton),
}

impl From<GamepadAxis> for GamepadInput {
    fn from(axis: GamepadAxis) -> Self {
        GamepadInput::Axis(axis)
    }
}

impl From<GamepadButton> for GamepadInput {
    fn from(button: GamepadButton) -> Self {
        GamepadInput::Button(button)
    }
}

#[derive(Debug, Clone, Copy)]
pub enum GamepadInputEvent {
    ButtonPressed(GamepadButton),
    ButtonRepeated(GamepadButton),
    ButtonReleased(GamepadButton),
    ButtonChanged { button: GamepadButton, value: f32 },
    AxisChanged { axis: GamepadAxis, value: f32 },
}
