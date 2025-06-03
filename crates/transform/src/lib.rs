use ecs::{Component, ComponentKit};
use math::{Affine3A, Vec2, Vec3};

pub mod plugin;

#[derive(Component)]
pub struct Transform {
    pub translation: Vec3,
    pub scale: Vec3,
    pub rotation: Vec3,
}

#[derive(Component)]
pub struct Transform2d {
    pub translation: Vec2,
    pub scale: Vec2,
    pub rotation: f32,
}

#[derive(Component)]
pub struct GlobalTransform(Affine3A);
impl std::ops::Deref for GlobalTransform {
    type Target = Affine3A;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(ComponentKit)]
pub struct TransformKit {
    /// The local transform is the transform relative to the parent entity.
    pub local: Transform,
    /// The global transform is the transform relative to the world origin.
    pub global: GlobalTransform,
}

#[derive(ComponentKit)]
pub struct Transform2dKit {
    /// The local transform is the transform relative to the parent entity.
    pub local: Transform2d,
    /// The global transform is the transform relative to the world origin.
    pub global: GlobalTransform,
}
