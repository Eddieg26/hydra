use serde::{Deserialize, Serialize};

#[derive(Default, Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Point2d {
    pub x: f32,
    pub y: f32,
}

impl Point2d {
    pub const ORIGIN: Point2d = Point2d { x: 0.0, y: 0.0 };

    pub fn new(x: f32, y: f32) -> Self {
        Point2d { x, y }
    }

    pub fn distance_to(&self, other: &Point2d) -> f32 {
        ((self.x - other.x).powi(2) + (self.y - other.y).powi(2)).sqrt()
    }

    pub fn translate(&mut self, dx: f32, dy: f32) {
        self.x += dx;
        self.y += dy;
    }
}

impl std::ops::Add for Point2d {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        Point2d {
            x: self.x + other.x,
            y: self.y + other.y,
        }
    }
}

impl std::ops::Sub for Point2d {
    type Output = Self;

    fn sub(self, other: Self) -> Self {
        Point2d {
            x: self.x - other.x,
            y: self.y - other.y,
        }
    }
}

impl std::ops::Mul<f32> for Point2d {
    type Output = Self;

    fn mul(self, scalar: f32) -> Self {
        Point2d {
            x: self.x * scalar,
            y: self.y * scalar,
        }
    }
}

impl std::ops::Div<f32> for Point2d {
    type Output = Self;

    fn div(self, scalar: f32) -> Self {
        if scalar == 0.0 {
            panic!("Division by zero is not allowed");
        }
        Point2d {
            x: self.x / scalar,
            y: self.y / scalar,
        }
    }
}

impl std::fmt::Display for Point2d {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Point2d({}, {})", self.x, self.y)
    }
}

#[derive(Default, Debug, Clone, Copy, PartialEq)]
pub struct Point3d {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Point3d {
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Point3d { x, y, z }
    }

    pub fn distance_to(&self, other: &Point3d) -> f32 {
        ((self.x - other.x).powi(2) + (self.y - other.y).powi(2) + (self.z - other.z).powi(2))
            .sqrt()
    }

    pub fn translate(&mut self, dx: f32, dy: f32, dz: f32) {
        self.x += dx;
        self.y += dy;
        self.z += dz;
    }
}

impl std::ops::Add for Point3d {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        Point3d {
            x: self.x + other.x,
            y: self.y + other.y,
            z: self.z + other.z,
        }
    }
}

impl std::ops::Sub for Point3d {
    type Output = Self;

    fn sub(self, other: Self) -> Self {
        Point3d {
            x: self.x - other.x,
            y: self.y - other.y,
            z: self.z - other.z,
        }
    }
}

impl std::ops::Mul<f32> for Point3d {
    type Output = Self;

    fn mul(self, scalar: f32) -> Self {
        Point3d {
            x: self.x * scalar,
            y: self.y * scalar,
            z: self.z * scalar,
        }
    }
}

impl std::ops::Div<f32> for Point3d {
    type Output = Self;

    fn div(self, scalar: f32) -> Self {
        if scalar == 0.0 {
            panic!("Division by zero is not allowed");
        }
        Point3d {
            x: self.x / scalar,
            y: self.y / scalar,
            z: self.z / scalar,
        }
    }
}

impl std::fmt::Display for Point3d {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Point3d({}, {}, {})", self.x, self.y, self.z)
    }
}
