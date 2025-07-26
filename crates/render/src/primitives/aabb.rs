use encase::ShaderType;
use math::{Vec3, bounds::Bounds};

#[derive(Debug, Clone, Copy, ShaderType, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct Aabb {
    pub min: Vec3,
    pub max: Vec3,
}

impl From<Bounds> for Aabb {
    fn from(value: Bounds) -> Self {
        Self {
            min: value.min,
            max: value.max,
        }
    }
}
