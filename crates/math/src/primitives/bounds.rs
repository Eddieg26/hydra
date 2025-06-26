use glam::{Affine3A, Mat4, Vec2, Vec3, Vec4, Vec4Swizzles};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Aabb {
    pub min: Vec3,
    pub max: Vec3,
}

impl Aabb {
    pub const ZERO: Self = Aabb::new(Vec3::ZERO, Vec3::ZERO);

    pub const fn new(min: Vec3, max: Vec3) -> Self {
        Aabb { min, max }
    }

    pub fn area(&self) -> f32 {
        (self.max.x - self.min.x) * (self.max.y - self.min.y)
    }

    pub fn intersects(&self, other: Aabb) -> bool {
        self.min.x <= other.max.x
            && self.max.x >= other.min.x
            && self.min.y <= other.max.y
            && self.max.y >= other.min.y
            && self.min.z <= other.max.z
            && self.max.z >= other.min.z
    }

    pub fn contains_point(&self, point: Vec3) -> bool {
        point.x >= self.min.x
            && point.y >= self.min.y
            && point.z >= self.min.z
            && point.x <= self.max.x
            && point.y <= self.max.y
            && point.z <= self.max.z
    }

    pub fn transform_affine(&self, matrix: &Affine3A) -> Self {
        let min = matrix.transform_point3(self.min);
        let max = matrix.transform_point3(self.max);

        Aabb {
            min: Vec3::new(min.x.min(max.x), min.y.min(max.y), min.z.min(max.z)),
            max: Vec3::new(min.x.max(max.x), min.y.max(max.y), min.z.max(max.z)),
        }
    }

    pub fn transform(&self, matrix: &Mat4) -> Self {
        let min = matrix.transform_point3(self.min);
        let max = matrix.transform_point3(self.max);

        Aabb {
            min: Vec3::new(min.x.min(max.x), min.y.min(max.y), min.z.min(max.z)),
            max: Vec3::new(min.x.max(max.x), min.y.max(max.y), min.z.max(max.z)),
        }
    }
}

impl std::ops::Add for Aabb {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        Aabb {
            min: Vec3::new(
                self.min.x + other.min.x,
                self.min.y + other.min.y,
                self.min.z + other.min.z,
            ),
            max: Vec3::new(
                self.max.x + other.max.x,
                self.max.y + other.max.y,
                self.max.z + other.max.z,
            ),
        }
    }
}

impl std::ops::Sub for Aabb {
    type Output = Self;

    fn sub(self, other: Self) -> Self {
        Aabb {
            min: Vec3::new(
                self.min.x - other.min.x,
                self.min.y - other.min.y,
                self.min.z - other.min.z,
            ),
            max: Vec3::new(
                self.max.x - other.max.x,
                self.max.y - other.max.y,
                self.max.z - other.max.z,
            ),
        }
    }
}

impl std::ops::Mul<f32> for Aabb {
    type Output = Self;

    fn mul(self, scalar: f32) -> Self {
        Aabb {
            min: Vec3::new(
                self.min.x * scalar,
                self.min.y * scalar,
                self.min.z * scalar,
            ),
            max: Vec3::new(
                self.max.x * scalar,
                self.max.y * scalar,
                self.max.z * scalar,
            ),
        }
    }
}

impl std::ops::Div<f32> for Aabb {
    type Output = Self;

    fn div(self, scalar: f32) -> Self {
        Aabb {
            min: Vec3::new(
                self.min.x / scalar,
                self.min.y / scalar,
                self.min.z / scalar,
            ),
            max: Vec3::new(
                self.max.x / scalar,
                self.max.y / scalar,
                self.max.z / scalar,
            ),
        }
    }
}

impl From<&[Vec3]> for Aabb {
    fn from(vertices: &[Vec3]) -> Self {
        let mut min = Vec3::splat(f32::INFINITY);
        let mut max = Vec3::splat(f32::NEG_INFINITY);

        for vertex in vertices {
            min = min.min(*vertex);
            max = max.max(*vertex);
        }

        Self { min, max }
    }
}

impl From<&[Vec2]> for Aabb {
    fn from(vertices: &[Vec2]) -> Self {
        let mut min = Vec3::splat(f32::INFINITY);
        let mut max = Vec3::splat(f32::NEG_INFINITY);

        for vertex in vertices {
            min = min.min(Vec3::new(vertex.x, vertex.y, 0.0));
            max = max.max(Vec3::new(vertex.x, vertex.y, 0.0));
        }

        Self { min, max }
    }
}

impl From<&[Vec4]> for Aabb {
    fn from(vertices: &[Vec4]) -> Self {
        let mut min = Vec3::splat(f32::INFINITY);
        let mut max = Vec3::splat(f32::NEG_INFINITY);

        for vertex in vertices {
            min = min.min(vertex.xyz());
            max = max.max(vertex.xyz());
        }

        Self { min, max }
    }
}
