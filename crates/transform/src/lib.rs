use math::{Affine3A, Vec2, Vec3};

pub struct Transform {
    pub translation: Vec3,
    pub scale: Vec3,
    pub rotation: Vec3,
}

pub struct Transform2d {
    pub translation: Vec2,
    pub scale: Vec2,
    pub rotation: f32,
}

pub struct GlobalTransform(Affine3A);
impl std::ops::Deref for GlobalTransform {
    type Target = Affine3A;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub struct TransformKit {
    /// The local transform is the transform relative to the parent entity.
    pub local: Transform,
    /// The global transform is the transform relative to the world origin.
    pub global: GlobalTransform,
}

pub struct Transform2dKit {
    /// The local transform is the transform relative to the parent entity.
    pub local: Transform2d,
    /// The global transform is the transform relative to the world origin.
    pub global: GlobalTransform,
}
