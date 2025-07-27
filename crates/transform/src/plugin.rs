use crate::{GlobalTransform, LocalTransform, update_global_transforms};
use ecs::{Children, Parent, Plugin, app::PostUpdate};

pub struct TransformPlugin<T: LocalTransform>(std::marker::PhantomData<T>);
impl<T: LocalTransform> TransformPlugin<T> {
    pub fn new() -> Self {
        TransformPlugin(std::marker::PhantomData)
    }
}

impl<T: LocalTransform> Plugin for TransformPlugin<T> {
    fn setup(&mut self, app: &mut ecs::AppBuilder) {
        app.register::<T>()
            .register::<GlobalTransform>()
            .register::<Parent>()
            .register::<Children>()
            .add_systems(PostUpdate, update_global_transforms::<T>);
    }
}
