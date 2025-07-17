use crate::{Buttons, button::ButtonState};
use ecs::{Event, EventReader, Resource};
use math::Vec2;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MouseButton {
    /// The left mouse button.
    Left,
    /// The right mouse button.
    Right,
    /// The middle mouse button.
    Middle,
    /// The back mouse button.
    Back,
    /// The forward mouse button.
    Forward,
    /// Another mouse button with the associated number.
    Other(u16),
}

#[derive(Event, Debug, Clone, Copy)]
pub struct MouseInput {
    /// The button that was pressed or released.
    pub button: MouseButton,
    /// The state of the button (pressed or released).
    pub state: ButtonState,
}

impl MouseInput {
    pub(crate) fn process_events(
        mouse_buttons: &mut Buttons<MouseButton>,
        events: EventReader<MouseInput>,
    ) {
        for event in events {
            match event.state {
                ButtonState::Pressed => {
                    mouse_buttons.set(event.button, ButtonState::Pressed);
                }
                ButtonState::Released => {
                    mouse_buttons.set(event.button, ButtonState::Released);
                }
            }
        }
    }
}

#[derive(Debug, Clone, Copy, Event)]
pub struct MouseMove {
    /// The change in the x position of the mouse.
    pub x: f32,
    /// The change in the y position of the mouse.
    pub y: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct MousePosition {
    /// The x position of the mouse.
    pub x: f32,
    /// The y position of the mouse.
    pub y: f32,
}

#[derive(Debug, Clone, Copy)]
pub enum MouseScrollUnit {
    /// The scroll unit is in lines.
    Line,
    /// The scroll unit is in pixels.
    Pixel,
}

#[derive(Debug, Clone, Copy, Event)]
pub struct MouseScrollDelta {
    /// The unit of the scroll delta.
    pub unit: MouseScrollUnit,
    /// The change in the x position of the scroll.
    pub x: f32,
    /// The change in the y position of the scroll.
    pub y: f32,
}

#[derive(Debug, Clone, Copy, Event)]
pub struct MouseScroll {
    /// The scroll delta.
    pub delta: MouseScrollDelta,
}

#[derive(Debug, Clone, Copy, Resource)]
pub struct MouseScrollAccumulator {
    /// The accumulated scroll delta in lines.
    pub line: MouseScrollDelta,
    /// The accumulated scroll delta in pixels.
    pub pixel: MouseScrollDelta,
}

impl MouseScrollAccumulator {
    pub fn new() -> Self {
        Self {
            line: MouseScrollDelta {
                unit: MouseScrollUnit::Line,
                x: 0.0,
                y: 0.0,
            },
            pixel: MouseScrollDelta {
                unit: MouseScrollUnit::Pixel,
                x: 0.0,
                y: 0.0,
            },
        }
    }

    pub fn accumulate(&mut self, delta: &MouseScrollDelta) {
        match delta.unit {
            MouseScrollUnit::Line => {
                self.line.x += delta.x;
                self.line.y += delta.y;
            }
            MouseScrollUnit::Pixel => {
                self.pixel.x += delta.x;
                self.pixel.y += delta.y;
            }
        }
    }

    pub fn delta(&self, unit: &MouseScrollUnit) -> Vec2 {
        match unit {
            MouseScrollUnit::Line => Vec2 {
                x: self.line.x,
                y: self.line.y,
            },
            MouseScrollUnit::Pixel => Vec2 {
                x: self.pixel.x,
                y: self.pixel.y,
            },
        }
    }

    pub fn reset(&mut self) {
        self.line = MouseScrollDelta {
            unit: MouseScrollUnit::Line,
            x: 0.0,
            y: 0.0,
        };
        self.pixel = MouseScrollDelta {
            unit: MouseScrollUnit::Pixel,
            x: 0.0,
            y: 0.0,
        };
    }
}
