use ecs::Plugin;
use crate::{allocator::{MeshAllocator, MeshAllocatorConfig}, Process};

pub struct MeshAllocatorPlugin;

impl Plugin for MeshAllocatorPlugin {
    fn setup(&mut self, app: &mut ecs::AppBuilder) {
        app.add_resource(MeshAllocatorConfig::default());
    }

    fn finish(&mut self, app: &mut ecs::AppBuilder) {
        let config = app
            .remove_resource::<MeshAllocatorConfig>()
            .unwrap_or_default();
        app.add_resource(MeshAllocator::new(config));
        app.add_systems(Process, MeshAllocator::update);
    }
}
