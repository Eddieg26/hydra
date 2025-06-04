use crate::{
    app::{
        PostRender, PreRender, Present, Process, ProcessAssets, ProcessPipelines, ProcessResources,
        Queue, QueueDraws, QueueViews, Render, RenderApp,
    },
    surface::{RenderSurface, RenderSurfaceTexture},
};
use ecs::{Init, Plugin, Run};
use window::plugin::WindowPlugin;

pub struct RenderPlugin;

impl Plugin for RenderPlugin {
    fn setup(&mut self, app: &mut ecs::AppBuilder) {
        app.add_plugins(WindowPlugin)
            .add_sub_app(RenderApp)
            .add_sub_phase(Run, Process)
            .add_sub_phase(Process, ProcessResources)
            .add_sub_phase(Process, ProcessAssets)
            .add_sub_phase(Process, ProcessPipelines)
            .add_sub_phase(Run, Queue)
            .add_sub_phase(Queue, QueueViews)
            .add_sub_phase(Queue, QueueDraws)
            .add_sub_phase(Run, PreRender)
            .add_sub_phase(Run, Render)
            .add_sub_phase(Run, PostRender)
            .add_sub_phase(Run, Present)
            .add_systems(Init, RenderSurface::create_surface)
            .add_systems(Queue, RenderSurface::queue_surface)
            .add_systems(Present, RenderSurface::present_surface)
            .add_resource(RenderSurfaceTexture::new());
    }
}
