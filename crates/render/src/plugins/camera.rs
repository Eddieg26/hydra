use crate::{
    ActiveCamera, Camera, CameraAttachments, CameraPhase, CameraSortOrder, PostRender, PreRender,
    Render, RenderApp, plugins::RenderPlugin,
};
use ecs::{AppBuilder, Plugin, app::sync::SyncComponentPlugin};

pub struct CameraPlugin;
impl Plugin for CameraPlugin {
    fn setup(&mut self, app: &mut AppBuilder) {
        app.add_plugins((
            SyncComponentPlugin::<Camera, RenderApp>::new(),
            RenderPlugin,
        ))
        .sub_app_mut(RenderApp)
        .add_sub_phase(Render, CameraPhase)
        .register::<CameraAttachments>()
        .register::<ActiveCamera>()
        .add_resource(CameraSortOrder::default())
        .add_systems(PreRender, CameraAttachments::queue)
        .add_systems(PostRender, CameraAttachments::cleanup);
    }
}
