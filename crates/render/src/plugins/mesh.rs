use crate::{
    Mesh, Process, RenderApp, SubMesh,
    allocator::{MeshAllocator, MeshAllocatorConfig},
    plugins::RenderPlugin,
};
use asset::plugin::AssetAppExt;
use ecs::Plugin;

pub struct MeshPlugin;

impl Plugin for MeshPlugin {
    fn setup(&mut self, app: &mut ecs::AppBuilder) {
        app.add_plugins(RenderPlugin)
            .register_asset::<Mesh>()
            .register_asset::<SubMesh>()
            .add_loader::<SubMesh>()
            .add_resource(MeshAllocatorConfig::default());
    }

    fn finish(&mut self, app: &mut ecs::AppBuilder) {
        let config = app
            .remove_resource::<MeshAllocatorConfig>()
            .unwrap_or_default();

        app.sub_app_mut(RenderApp)
            .add_resource(MeshAllocator::new(config))
            .add_systems(Process, MeshAllocator::update);
    }
}
