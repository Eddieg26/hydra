use ecs::{End, EventWriter, Plugin, Resource};
use input::{GamepadAxis, GamepadEvent, GamepadId, GamepadInputEvent, plugin::GamepadPlugin};

pub mod translate;

#[derive(Resource)]
pub struct Gilrs(pub(crate) gilrs::Gilrs);

impl Gilrs {
    pub fn new() -> Result<Self, gilrs::Error> {
        gilrs::Gilrs::new().map(Self)
    }

    pub fn process_events(gilrs: &mut Gilrs, mut events: EventWriter<GamepadEvent>) {
        while let Some(gilrs::Event { id, event, .. }) = gilrs.0.next_event() {
            let id = GamepadId::from(usize::from(id));
            match event {
                gilrs::EventType::ButtonPressed(button, _) => events.send(GamepadEvent::Input {
                    id,
                    value: GamepadInputEvent::ButtonPressed(translate::gamepad_button(button)),
                }),
                gilrs::EventType::ButtonRepeated(button, _) => events.send(GamepadEvent::Input {
                    id,
                    value: GamepadInputEvent::ButtonRepeated(translate::gamepad_button(button)),
                }),
                gilrs::EventType::ButtonReleased(button, _) => events.send(GamepadEvent::Input {
                    id,
                    value: GamepadInputEvent::ButtonReleased(translate::gamepad_button(button)),
                }),
                gilrs::EventType::ButtonChanged(button, value, _) => {
                    events.send(GamepadEvent::Input {
                        id,
                        value: GamepadInputEvent::ButtonChanged {
                            button: translate::gamepad_button(button),
                            value,
                        },
                    })
                }
                gilrs::EventType::AxisChanged(axis, value, _) => events.send(GamepadEvent::Input {
                    id,
                    value: GamepadInputEvent::AxisChanged {
                        axis: GamepadAxis::from(translate::gamepad_axis(axis)),
                        value,
                    },
                }),
                gilrs::EventType::Connected => events.send(GamepadEvent::Connected {
                    id,
                    name: String::new(),
                    vendor_id: None,
                    product_id: None,
                }),
                gilrs::EventType::Disconnected => events.send(GamepadEvent::Disconnected(id)),
                _ => continue,
            }
        }
    }
}

impl std::ops::Deref for Gilrs {
    type Target = gilrs::Gilrs;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub struct GilrsPlugin;

impl Plugin for GilrsPlugin {
    fn setup(&mut self, app: &mut ecs::AppBuilder) {
        match Gilrs::new() {
            Ok(gilrs) => app
                .add_plugins(GamepadPlugin)
                .add_resource(gilrs)
                .add_systems(End, Gilrs::process_events),
            Err(error) => unreachable!("{error}"),
        };
    }
}
