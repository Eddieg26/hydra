use crate::{analog::Analogs, button::Buttons};
use ecs::{Event, EventWriter, Resource};
use gilrs::GamepadId;
use std::collections::HashMap;

#[derive(Resource)]
pub struct Gilrs(gilrs::Gilrs);

impl Gilrs {
    pub fn new() -> Result<Self, gilrs::Error> {
        gilrs::Gilrs::new().map(Self)
    }
}

impl AsRef<gilrs::Gilrs> for Gilrs {
    fn as_ref(&self) -> &gilrs::Gilrs {
        &self.0
    }
}

impl AsMut<gilrs::Gilrs> for Gilrs {
    fn as_mut(&mut self) -> &mut gilrs::Gilrs {
        &mut self.0
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GamepadStatus {
    Connected,
    Disconnected,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Event)]
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

    pub fn disconnect(&mut self, id: GamepadId) {
        if let Some(pad) = self.gamepads.get_mut(&id) {
            pad.status = GamepadStatus::Disconnected;
        }
    }

    pub fn get(&self, id: &GamepadId) -> Option<&Gamepad> {
        self.gamepads.get(id)
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

    pub(crate) fn process_events(
        gamepads: &mut Gamepads,
        gilrs: &mut Gilrs,
        mut events: EventWriter<GamepadEvent>,
    ) {
        while let Some(gilrs::Event { id, event, .. }) = gilrs.as_mut().next_event() {
            match event {
                gilrs::EventType::ButtonPressed(button, code) => todo!(),
                gilrs::EventType::ButtonRepeated(button, code) => todo!(),
                gilrs::EventType::ButtonReleased(button, code) => todo!(),
                gilrs::EventType::ButtonChanged(button, _, code) => todo!(),
                gilrs::EventType::AxisChanged(axis, _, code) => todo!(),
                gilrs::EventType::Connected => {
                    let connected = gilrs.as_mut().gamepad(id);
                    let gamepad = Gamepad {
                        status: GamepadStatus::Connected,
                        vendor_id: connected.vendor_id(),
                        product_id: connected.product_id(),
                        buttons: Buttons::new(),
                        analogs: Analogs::new(),
                    };

                    gamepads.connect(id, gamepad);
                    events.send(GamepadEvent::Connected {
                        id,
                        name: connected.name().to_string(),
                        vendor_id: connected.vendor_id(),
                        product_id: connected.product_id(),
                    });
                }
                gilrs::EventType::Disconnected => {
                    gamepads.disconnect(id);
                    events.send(GamepadEvent::Disconnected(id));
                }
                gilrs::EventType::ForceFeedbackEffectCompleted => todo!(),
                _ => todo!(),
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

// let v = gilrs::Gilrs::new().unwrap();
// let e = v.next_event().unwrap();
// match e.event {
//     gilrs::EventType::ButtonPressed(button, code) => todo!(),
//     gilrs::EventType::ButtonRepeated(button, code) => todo!(),
//     gilrs::EventType::ButtonReleased(button, code) => todo!(),
//     gilrs::EventType::ButtonChanged(button, v, code) => todo!(),
//     gilrs::EventType::AxisChanged(axis, v, code) => todo!(),
//     gilrs::EventType::Connected => todo!(),
//     gilrs::EventType::Disconnected => todo!(),
//     gilrs::EventType::Dropped => todo!(),
//     gilrs::EventType::ForceFeedbackEffectCompleted => todo!(),
//     _ => todo!(),
// }
