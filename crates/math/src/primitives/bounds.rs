use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Bounds {
    pub min: glam::Vec2,
    pub max: glam::Vec2,
}

impl Bounds {
    pub fn new(min: glam::Vec2, max: glam::Vec2) -> Self {
        Bounds { min, max }
    }

    pub fn area(&self) -> f32 {
        (self.max.x - self.min.x) * (self.max.y - self.min.y)
    }

    pub fn contains_point(&self, point: glam::Vec2) -> bool {
        point.x >= self.min.x
            && point.x <= self.max.x
            && point.y >= self.min.y
            && point.y <= self.max.y
    }
}

impl std::ops::Add for Bounds {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        Bounds {
            min: glam::Vec2::new(self.min.x + other.min.x, self.min.y + other.min.y),
            max: glam::Vec2::new(self.max.x + other.max.x, self.max.y + other.max.y),
        }
    }
}

impl std::ops::Sub for Bounds {
    type Output = Self;

    fn sub(self, other: Self) -> Self {
        Bounds {
            min: glam::Vec2::new(self.min.x - other.min.x, self.min.y - other.min.y),
            max: glam::Vec2::new(self.max.x - other.max.x, self.max.y - other.max.y),
        }
    }
}

impl std::ops::Mul<f32> for Bounds {
    type Output = Self;

    fn mul(self, scalar: f32) -> Self {
        Bounds {
            min: self.min * scalar,
            max: self.max * scalar,
        }
    }
}

impl std::ops::Div<f32> for Bounds {
    type Output = Self;

    fn div(self, scalar: f32) -> Self {
        Bounds {
            min: self.min / scalar,
            max: self.max / scalar,
        }
    }
}
