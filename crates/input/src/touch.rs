use ecs::Event;
use math::Vec2;

#[derive(Debug, Hash, PartialEq, Eq, Clone, Copy)]
pub enum TouchPhase {
    Started,
    Moved,
    Ended,
    Cancelled,
}

/// Describes the force of a touch event
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TouchForce {
    /// On iOS, the force is calibrated so that the same number corresponds to
    /// roughly the same amount of pressure on the screen regardless of the
    /// device.
    Calibrated {
        /// The force of the touch, where a value of 1.0 represents the force of
        /// an average touch (predetermined by the system, not user-specific).
        ///
        /// The force reported by Apple Pencil is measured along the axis of the
        /// pencil. If you want a force perpendicular to the device, you need to
        /// calculate this value using the `altitude_angle` value.
        force: f64,
        /// The maximum possible force for a touch.
        ///
        /// The value of this field is sufficiently high to provide a wide
        /// dynamic range for values of the `force` field.
        max_possible_force: f64,
        /// The altitude (in radians) of the stylus.
        ///
        /// A value of 0 radians indicates that the stylus is parallel to the
        /// surface. The value of this property is Pi/2 when the stylus is
        /// perpendicular to the surface.
        altitude_angle: Option<f64>,
    },
    /// If the platform reports the force as normalized, we have no way of
    /// knowing how much pressure 1.0 corresponds to – we know it's the maximum
    /// amount of force, but as to how much force, you might either have to
    /// press really really hard, or not hard at all, depending on the device.
    Normalized(f64),
}

#[derive(Event)]
pub struct PinchGesture {
    pub delta: f64,
    pub phase: TouchPhase,
}

impl PinchGesture {
    pub fn new(delta: f64, phase: TouchPhase) -> Self {
        Self { delta, phase }
    }
}

#[derive(Event)]
pub struct PanGesture {
    pub delta: Vec2,
    pub phase: TouchPhase,
}

impl PanGesture {
    pub fn new(x: f32, y: f32, phase: TouchPhase) -> Self {
        Self {
            delta: Vec2 { x, y },
            phase,
        }
    }
}

#[derive(Event)]
pub struct DoubleTapGesture;

#[derive(Event)]
pub struct RotationGesture {
    pub delta: f32,
    pub phase: TouchPhase,
}

impl RotationGesture {
    pub fn new(delta: f32, phase: TouchPhase) -> Self {
        Self { delta, phase }
    }
}

#[derive(Event)]
pub struct TouchpadPressure {
    pub pressure: f32,
    pub stage: i64,
}

impl TouchpadPressure {
    pub fn new(pressure: f32, stage: i64) -> Self {
        Self { pressure, stage }
    }
}

#[derive(Event)]
pub struct TouchInput {
    /// Unique identifier of a finger.
    pub id: u64,
    /// The phase of the touch event.
    pub phase: TouchPhase,
    pub position: Vec2,
    /// Describes how hard the screen was pressed. May be `None` if the platform
    /// does not support pressure sensitivity.
    ///
    /// ## Platform-specific
    ///
    /// - Only available on **iOS** 9.0+, **Windows** 8+, **Web**, and **Android**.
    /// - **Android**: This will never be [None]. If the device doesn't support pressure
    ///   sensitivity, force will either be 0.0 or 1.0. Also see the
    ///   [android documentation](https://developer.android.com/reference/android/view/MotionEvent#AXIS_PRESSURE).
    pub force: Option<TouchForce>,
}

impl TouchInput {
    pub fn new(id: u64, phase: TouchPhase, x: f32, y: f32, force: Option<TouchForce>) -> Self {
        Self {
            id,
            phase,
            position: Vec2 { x, y },
            force,
        }
    }
}
