use ecs::{
    Component, ComponentKit, Entity, HierarchyExt, Parent, Query,
    query::{With, Without},
};
use math::{Affine3A, Mat4, Quat, Vec2, Vec3, Vec3A};

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

impl GlobalTransform {
    pub const IDENTITY: Self = GlobalTransform(Affine3A::IDENTITY);

    pub fn new(translation: Vec3, rotation: math::Quat, scale: Vec3) -> Self {
        GlobalTransform(Affine3A::from_scale_rotation_translation(
            scale,
            rotation,
            translation,
        ))
    }

    pub fn from_affine(affine: Affine3A) -> Self {
        GlobalTransform(affine)
    }

    pub fn get(&self) -> &Affine3A {
        &self.0
    }

    pub fn affine(&self) -> Affine3A {
        self.0
    }
    
    pub fn matrix(&self) -> Mat4 {
        Mat4::from(self.0)
    }

    pub fn translation(&self) -> Vec3 {
        self.0.translation.into()
    }

    pub fn affine_translation(&self) -> Vec3A {
        self.0.translation
    }

    pub fn transform_point(&self, point: Vec3) -> Vec3 {
        self.0.transform_point3(point)
    }

    pub fn to_scale_rotation_translation(&self) -> (Vec3, Quat, Vec3) {
        self.0.to_scale_rotation_translation()
    }
}

impl<L: LocalTransform> std::ops::Mul<&L> for &GlobalTransform {
    type Output = GlobalTransform;

    fn mul(self, local_transform: &L) -> Self::Output {
        GlobalTransform(self.0 * local_transform.local())
    }
}

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

pub trait LocalTransform: Component {
    fn local(&self) -> Affine3A;
}

impl LocalTransform for Transform {
    fn local(&self) -> Affine3A {
        let rotation = math::Quat::from_euler(
            math::EulerRot::XYZ,
            self.rotation.x,
            self.rotation.y,
            self.rotation.z,
        );
        Affine3A::from_scale_rotation_translation(self.scale, rotation, self.translation)
    }
}

impl LocalTransform for Transform2d {
    fn local(&self) -> Affine3A {
        let rotation = math::Quat::from_rotation_z(self.rotation);
        Affine3A::from_scale_rotation_translation(
            Vec3::new(self.scale.x, self.scale.y, 1.0),
            rotation,
            Vec3::new(self.translation.x, self.translation.y, 0.0),
        )
    }
}

pub(crate) fn update_global_transforms<L: LocalTransform>(
    root_query: Query<(Entity, &L, &mut GlobalTransform), Without<Parent>>,
    transform_query: Query<(Entity, &L, &mut GlobalTransform), With<Parent>>,
) {
    for (entity, local_transform, global_transform) in root_query.iter() {
        // Calculate the global transform based on the local transform
        *global_transform = GlobalTransform(local_transform.local());

        update_child_transforms(entity, global_transform, &transform_query);
    }
}

fn update_child_transforms<L: LocalTransform>(
    entity: Entity,
    global_transform: &GlobalTransform,
    query: &Query<(Entity, &L, &mut GlobalTransform), With<Parent>>,
) {
    for (child, child_transform, child_global_transform) in query.children(entity) {
        *child_global_transform = global_transform * child_transform;
        update_child_transforms(child, &child_global_transform, query);
    }
}
