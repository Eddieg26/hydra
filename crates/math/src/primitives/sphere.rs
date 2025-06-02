use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Sphere {
    pub center: glam::Vec3,
    pub radius: f32,
}

impl Sphere {
    pub fn new(center: glam::Vec3, radius: f32) -> Self {
        Sphere { center, radius }
    }

    pub fn volume(&self) -> f32 {
        (4.0 / 3.0) * std::f32::consts::PI * self.radius.powi(3)
    }

    pub fn surface_area(&self) -> f32 {
        4.0 * std::f32::consts::PI * self.radius.powi(2)
    }

    pub fn contains_point(&self, point: glam::Vec3) -> bool {
        (point - self.center).length_squared() <= self.radius.powi(2)
    }
}

impl std::ops::Add for Sphere {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        Sphere {
            center: self.center + other.center,
            radius: self.radius + other.radius,
        }
    }
}

impl std::ops::Sub for Sphere {
    type Output = Self;

    fn sub(self, other: Self) -> Self {
        Sphere {
            center: self.center - other.center,
            radius: (self.radius - other.radius).max(0.0),
        }
    }
}

impl std::ops::Mul<f32> for Sphere {
    type Output = Self;

    fn mul(self, scalar: f32) -> Self {
        Sphere {
            center: self.center * scalar,
            radius: self.radius * scalar,
        }
    }
}

impl std::ops::Div<f32> for Sphere {
    type Output = Self;

    fn div(self, scalar: f32) -> Self {
        if scalar == 0.0 {
            panic!("Division by zero is not allowed for Sphere.");
        }
        Sphere {
            center: self.center / scalar,
            radius: self.radius / scalar,
        }
    }
}

impl std::fmt::Display for Sphere {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Sphere {{ center: {:?}, radius: {} }}",
            self.center, self.radius
        )
    }
}
