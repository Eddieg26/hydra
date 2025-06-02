use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Circle {
    pub center: glam::Vec2,
    pub radius: f32,
}

impl Circle {
    pub fn new(center: glam::Vec2, radius: f32) -> Self {
        Circle { center, radius }
    }

    pub fn area(&self) -> f32 {
        std::f32::consts::PI * self.radius * self.radius
    }

    pub fn circumference(&self) -> f32 {
        2.0 * std::f32::consts::PI * self.radius
    }

    pub fn contains_point(&self, point: glam::Vec2) -> bool {
        (point - self.center).length_squared() <= self.radius * self.radius
    }
}

impl std::ops::Add for Circle {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        Circle {
            center: self.center + other.center,
            radius: self.radius + other.radius,
        }
    }
}

impl std::ops::Sub for Circle {
    type Output = Self;

    fn sub(self, other: Self) -> Self {
        Circle {
            center: self.center - other.center,
            radius: (self.radius - other.radius).max(0.0),
        }
    }
}

impl std::ops::Mul<f32> for Circle {
    type Output = Self;

    fn mul(self, scalar: f32) -> Self {
        Circle {
            center: self.center * scalar,
            radius: self.radius * scalar,
        }
    }
}

impl std::ops::Div<f32> for Circle {
    type Output = Self;

    fn div(self, scalar: f32) -> Self {
        if scalar == 0.0 {
            panic!("Division by zero is not allowed for Circle.");
        }
        Circle {
            center: self.center / scalar,
            radius: self.radius / scalar,
        }
    }
}

impl std::fmt::Display for Circle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Circle {{ center: {:?}, radius: {} }}",
            self.center, self.radius
        )
    }
}
