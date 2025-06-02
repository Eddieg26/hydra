use crate::{
    keyboard::KeyCode,
    winit::{
        AxisId, DeviceId, ElementState, Force, KeyLocation, Modifiers, MouseButton,
        PhysicalPosition, PhysicalSize, SmolStr, TouchPhase, WindowId,
    },
};
use ecs::Event;
use std::path::PathBuf;

#[derive(Event)]
pub struct WindowCreated {
    pub id: WindowId,
}

impl WindowCreated {
    pub fn new(id: WindowId) -> Self {
        Self { id }
    }
}

#[derive(Event)]
pub struct WindowClosed {
    pub id: WindowId,
}

impl WindowClosed {
    pub fn new(id: WindowId) -> Self {
        Self { id }
    }
}

#[derive(Event)]
pub struct WindowDestroyed {
    pub id: WindowId,
}

impl WindowDestroyed {
    pub fn new(id: WindowId) -> Self {
        Self { id }
    }
}

#[derive(Event)]
pub struct ScaleFactorChanged {
    pub scale_factor: f64,
}

impl ScaleFactorChanged {
    pub fn new(scale_factor: f64) -> Self {
        Self { scale_factor }
    }
}

#[derive(Event)]
pub struct WindowOccluded {
    pub occluded: bool,
}

impl WindowOccluded {
    pub fn new(occluded: bool) -> Self {
        Self { occluded }
    }
}

#[derive(Event)]
pub struct WindowFocused {
    pub focused: bool,
}

impl WindowFocused {
    pub fn new(focused: bool) -> Self {
        Self { focused }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Event)]
pub struct WindowResized {
    pub size: PhysicalSize<u32>,
}

impl WindowResized {
    pub fn new(size: PhysicalSize<u32>) -> Self {
        Self { size }
    }

    pub fn width(&self) -> u32 {
        self.size.width
    }

    pub fn height(&self) -> u32 {
        self.size.height
    }
}

#[derive(Event)]
pub struct WindowMoved {
    pub position: PhysicalPosition<i32>,
}

impl WindowMoved {
    pub fn new(position: PhysicalPosition<i32>) -> Self {
        Self { position }
    }
}

#[derive(Event)]
pub struct DroppedFile {
    pub path: PathBuf,
}

impl DroppedFile {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }
}

#[derive(Event)]
pub struct HoveredFile {
    pub path: PathBuf,
}

impl HoveredFile {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }
}

#[derive(Event)]
pub struct HoveredFileCancelled;

#[derive(Event)]
pub struct KeyEvent {
    pub device: DeviceId,
    pub code: KeyCode,
    pub text: Option<SmolStr>,
    pub location: KeyLocation,
    pub state: ElementState,
    pub repeat: bool,
    pub synthetic: bool,
}

impl KeyEvent {
    pub fn new(device: DeviceId, event: winit::event::KeyEvent, synthetic: bool) -> Self {
        Self {
            device,
            code: KeyCode::from(event.physical_key),
            text: event.text,
            location: event.location,
            state: event.state,
            repeat: event.repeat,
            synthetic,
        }
    }
}

#[derive(Event)]
pub struct ModifiersChanged {
    pub modifiers: Modifiers,
}

impl ModifiersChanged {
    pub fn new(modifiers: Modifiers) -> Self {
        Self { modifiers }
    }
}

#[derive(Event)]
pub struct CursorMoved {
    pub device: DeviceId,
    pub position: PhysicalPosition<f64>,
}

impl CursorMoved {
    pub fn new(device: DeviceId, position: PhysicalPosition<f64>) -> Self {
        Self { device, position }
    }
}

#[derive(Event)]
pub struct CursorEntered {
    pub device: DeviceId,
}

impl CursorEntered {
    pub fn new(device: DeviceId) -> Self {
        Self { device }
    }
}

#[derive(Event)]
pub struct CursorLeft {
    pub device: DeviceId,
}

impl CursorLeft {
    pub fn new(device: DeviceId) -> Self {
        Self { device }
    }
}

#[derive(Event)]
pub struct AxisMotion {
    pub device: DeviceId,
    pub axis: AxisId,
    pub value: f64,
}

impl AxisMotion {
    pub fn new(device: DeviceId, axis: AxisId, value: f64) -> Self {
        Self {
            device,
            axis,
            value,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MouseScrollDelta {
    Line(PhysicalPosition<f32>),
    Pixel(PhysicalPosition<f64>),
}

impl From<winit::event::MouseScrollDelta> for MouseScrollDelta {
    fn from(delta: winit::event::MouseScrollDelta) -> Self {
        match delta {
            winit::event::MouseScrollDelta::LineDelta(x, y) => {
                MouseScrollDelta::Line(PhysicalPosition::new(x, y))
            }
            winit::event::MouseScrollDelta::PixelDelta(pos) => MouseScrollDelta::Pixel(pos),
        }
    }
}

#[derive(Event)]
pub struct MouseScroll {
    pub device: DeviceId,
    pub delta: MouseScrollDelta,
    pub phase: TouchPhase,
}

impl MouseScroll {
    pub fn new(device: DeviceId, delta: impl Into<MouseScrollDelta>, phase: TouchPhase) -> Self {
        Self {
            device,
            delta: delta.into(),
            phase,
        }
    }
}

#[derive(Event)]
pub struct MouseInput {
    pub device: DeviceId,
    pub state: ElementState,
    pub button: MouseButton,
}

impl MouseInput {
    pub fn new(device: DeviceId, state: ElementState, button: MouseButton) -> Self {
        Self {
            device,
            state,
            button,
        }
    }
}

#[derive(Event)]
pub struct PinchGesture {
    pub device: DeviceId,
    pub delta: f64,
    pub phase: TouchPhase,
}

impl PinchGesture {
    pub fn new(device: DeviceId, delta: f64, phase: TouchPhase) -> Self {
        Self {
            device,
            delta,
            phase,
        }
    }
}

#[derive(Event)]
pub struct PanGesture {
    pub device: DeviceId,
    pub delta: PhysicalPosition<f32>,
    pub phase: TouchPhase,
}

impl PanGesture {
    pub fn new(device: DeviceId, delta: PhysicalPosition<f32>, phase: TouchPhase) -> Self {
        Self {
            device,
            delta,
            phase,
        }
    }
}

#[derive(Event)]
pub struct DoubleTapGesture {
    pub device: DeviceId,
}

impl DoubleTapGesture {
    pub fn new(device: DeviceId) -> Self {
        Self { device }
    }
}

#[derive(Event)]
pub struct RotationGesture {
    pub device: DeviceId,
    pub delta: f32,
    pub phase: TouchPhase,
}

impl RotationGesture {
    pub fn new(device: DeviceId, delta: f32, phase: TouchPhase) -> Self {
        Self {
            device,
            delta,
            phase,
        }
    }
}

#[derive(Event)]
pub struct TouchpadPressure {
    pub device: DeviceId,
    pub pressure: f32,
    pub stage: i64,
}

impl TouchpadPressure {
    pub fn new(device: DeviceId, pressure: f32, stage: i64) -> Self {
        Self {
            device,
            pressure,
            stage,
        }
    }
}

#[derive(Event)]
pub struct TouchEvent {
    pub device: DeviceId,
    pub phase: TouchPhase,
    pub location: PhysicalPosition<f64>,
    /// Describes how hard the screen was pressed. May be `None` if the platform
    /// does not support pressure sensitivity.
    ///
    /// ## Platform-specific
    ///
    /// - Only available on **iOS** 9.0+, **Windows** 8+, **Web**, and **Android**.
    /// - **Android**: This will never be [None]. If the device doesn't support pressure
    ///   sensitivity, force will either be 0.0 or 1.0. Also see the
    ///   [android documentation](https://developer.android.com/reference/android/view/MotionEvent#AXIS_PRESSURE).
    pub force: Option<Force>,
    /// Unique identifier of a finger.
    pub id: u64,
}

impl From<winit::event::Touch> for TouchEvent {
    fn from(touch: winit::event::Touch) -> Self {
        Self {
            device: touch.device_id,
            phase: touch.phase,
            location: touch.location,
            force: touch.force,
            id: touch.id,
        }
    }
}
