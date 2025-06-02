use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Rect {
    pub const ZERO: Rect = Rect {
        x: 0.0,
        y: 0.0,
        width: 0.0,
        height: 0.0,
    };

    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Rect {
            x,
            y,
            width,
            height,
        }
    }

    pub fn area(&self) -> f32 {
        self.width * self.height
    }

    pub fn contains_point(&self, point: glam::Vec2) -> bool {
        point.x >= self.x
            && point.x <= self.x + self.width
            && point.y >= self.y
            && point.y <= self.y + self.height
    }
}

impl std::ops::Add for Rect {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        Rect {
            x: self.x + other.x,
            y: self.y + other.y,
            width: self.width + other.width,
            height: self.height + other.height,
        }
    }
}

impl std::ops::Sub for Rect {
    type Output = Self;

    fn sub(self, other: Self) -> Self {
        Rect {
            x: self.x - other.x,
            y: self.y - other.y,
            width: self.width - other.width,
            height: self.height - other.height,
        }
    }
}

impl std::ops::Mul<f32> for Rect {
    type Output = Self;

    fn mul(self, scalar: f32) -> Self {
        Rect {
            x: self.x * scalar,
            y: self.y * scalar,
            width: self.width * scalar,
            height: self.height * scalar,
        }
    }
}

impl std::ops::Div<f32> for Rect {
    type Output = Self;

    fn div(self, scalar: f32) -> Self {
        Rect {
            x: self.x / scalar,
            y: self.y / scalar,
            width: self.width / scalar,
            height: self.height / scalar,
        }
    }
}
