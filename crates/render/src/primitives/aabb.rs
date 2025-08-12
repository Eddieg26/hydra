use encase::ShaderType;
use math::{Vec2, Vec3, Vec4, Vec4Swizzles};

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
