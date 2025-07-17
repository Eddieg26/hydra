use crate::winit::{AxisId, DeviceId, Modifiers, PhysicalPosition, PhysicalSize, WindowId};
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
