use input::{GamepadAxis, GamepadButton};

pub fn gamepad_button(button: gilrs::Button) -> GamepadButton {
    match button {
        gilrs::Button::South => GamepadButton::South,
        gilrs::Button::East => GamepadButton::East,
        gilrs::Button::North => GamepadButton::North,
        gilrs::Button::West => GamepadButton::West,
        gilrs::Button::C => GamepadButton::C,
        gilrs::Button::Z => GamepadButton::Z,
        gilrs::Button::LeftTrigger => GamepadButton::LeftTrigger,
        gilrs::Button::LeftTrigger2 => GamepadButton::LeftTrigger2,
        gilrs::Button::RightTrigger => GamepadButton::RightTrigger,
        gilrs::Button::RightTrigger2 => GamepadButton::RightTrigger2,
        gilrs::Button::Select => GamepadButton::Select,
        gilrs::Button::Start => GamepadButton::Start,
        gilrs::Button::Mode => GamepadButton::Mode,
        gilrs::Button::LeftThumb => GamepadButton::LeftThumb,
        gilrs::Button::RightThumb => GamepadButton::RightThumb,
        gilrs::Button::DPadUp => GamepadButton::DPadUp,
        gilrs::Button::DPadDown => GamepadButton::DPadDown,
        gilrs::Button::DPadLeft => GamepadButton::DPadLeft,
        gilrs::Button::DPadRight => GamepadButton::DPadRight,
        gilrs::Button::Unknown => GamepadButton::Unknown,
    }
}

pub fn gamepad_axis(axis: gilrs::Axis) -> GamepadAxis {
    match axis {
        gilrs::Axis::LeftStickX => GamepadAxis::LeftStickX,
        gilrs::Axis::LeftStickY => GamepadAxis::LeftStickY,
        gilrs::Axis::LeftZ => GamepadAxis::LeftZ,
        gilrs::Axis::RightStickX => GamepadAxis::RightZ,
        gilrs::Axis::RightStickY => GamepadAxis::RightStickX,
        gilrs::Axis::RightZ => GamepadAxis::RightStickY,
        gilrs::Axis::DPadX => GamepadAxis::DPadX,
        gilrs::Axis::DPadY => GamepadAxis::DPadY,
        gilrs::Axis::Unknown => GamepadAxis::Unknown,
    }
}
