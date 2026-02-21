use encase::ShaderType;
use math::{Mat4, Vec2, Vec3, Vec3Swizzles, Vec4, Vec4Swizzles, sphere::Sphere};

#[derive(
    Debug,
    Clone,
    Copy,
    ShaderType,
    bytemuck::Pod,
    bytemuck::Zeroable,
    serde::Serialize,
    serde::Deserialize,
)]
#[repr(C)]
pub struct Aabb {
    pub min: Vec3,
    pub max: Vec3,
}

impl Aabb {
    pub const ZERO: Self = Aabb::new(Vec3::ZERO, Vec3::ZERO);

    pub const fn new(min: Vec3, max: Vec3) -> Self {
        Aabb { min, max }
    }

    pub fn vertices(&self) -> [Vec3; 8] {
        [
            self.min,
            Vec3::new(self.max.x, self.min.y, self.min.z),
            Vec3::new(self.min.x, self.max.y, self.min.z),
            Vec3::new(self.min.x, self.min.y, self.max.z),
            Vec3::new(self.max.x, self.max.y, self.min.z),
            Vec3::new(self.max.x, self.min.y, self.max.z),
            Vec3::new(self.min.x, self.max.y, self.max.z),
            self.max,
        ]
    }

    pub fn sphere(&self) -> Sphere {
        let center = (self.min + self.max) / 2.0;
        let radius = (self.min.x - self.max.x)
            .abs()
            .max((self.min.y - self.max.y).abs())
            .max((self.min.z - self.max.z).abs());

        Sphere::new(center, radius)
    }

    pub fn transform(&self, matrix: Mat4) -> Aabb {
        let center = (self.min + self.max) / 2.0;
        let extent = (self.max - self.min) / 2.0;

        let center = matrix.transform_point3(center);
        let extent = matrix.transform_vector3(extent);

        let min = center.xyz() - extent.abs();
        let max = center.xyz() + extent.abs();

        Aabb::new(min, max)
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
