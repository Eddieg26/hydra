use crate::button::ButtonState;

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

pub struct MouseInput {
    pub button: MouseButton,
    pub state: ButtonState,
}

pub struct MouseDelta {
    /// The change in the x position of the mouse.
    pub x: f32,
    /// The change in the y position of the mouse.
    pub y: f32,
}

pub struct MousePosition {
    /// The x position of the mouse.
    pub x: f32,
    /// The y position of the mouse.
    pub y: f32,
}

pub enum MouseWheel {
    /// The scroll delta in lines.
    Line {
        /// The change in the x position of the scroll.
        x: f32,
        /// The change in the y position of the scroll.
        y: f32,
    },
    /// The scroll delta in pixels.
    Pixel {
        /// The change in the x position of the scroll.
        x: f64,
        /// The change in the y position of the scroll.
        y: f64,
    },
}
