use crate::{
    app::{
        PostRender, PreRender, Present, Process, ProcessPipelines, Queue, QueueDraws, QueueViews,
        Render, RenderApp,
    },
    resources::{
        AssetExtractors, PipelineCache, RenderAssetExtractor, RenderAssets, RenderResource,
        ResourceExtractors,
    },
    surface::{RenderSurface, RenderSurfaceTexture},
};
use ecs::{AppBuilder, Extract, Init, Plugin, Run};
use window::plugin::WindowPlugin;

pub struct RenderPlugin;

impl Plugin for RenderPlugin {
    fn setup(&mut self, app: &mut ecs::AppBuilder) {
        app.add_plugins(WindowPlugin)
            .add_sub_app(RenderApp)
            .add_sub_phase(Run, Process)
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
            .add_resource(RenderSurfaceTexture::new())
            .add_resource(AssetExtractors::default())
            .add_resource(ResourceExtractors::default())
            .add_resource(PipelineCache::default());
    }

    fn finish(&mut self, app: &mut AppBuilder) {
        if !app.resources().contains::<ResourceExtractors>() {
            app.add_resource(ResourceExtractors::default());
        }

        let app = app.sub_app_mut(RenderApp).unwrap();
        let configs = app
            .remove_resource::<AssetExtractors>()
            .map(|a| a.build())
            .unwrap_or_default();

        for config in configs {
            app.add_systems(Extract, config.extract);
            app.add_systems(Process, config.process);
            app.add_systems(PostRender, config.clear);
        }

        app.add_systems(Extract, ResourceExtractors::extract);
        app.add_systems(Process, ResourceExtractors::process);
        app.add_systems(ProcessPipelines, PipelineCache::process);
    }
}

pub trait RenderAppExt {
    fn extract_render_asset<R: RenderAssetExtractor>(&mut self) -> &mut Self;
    fn extract_render_resource<R: RenderResource>(&mut self) -> &mut Self;
}

impl RenderAppExt for AppBuilder {
    fn extract_render_asset<R: RenderAssetExtractor>(&mut self) -> &mut Self {
        let app = self.sub_app_mut(RenderApp).unwrap();
        app.resource_mut::<AssetExtractors>().add::<R>();

        if !app.resources().contains::<RenderAssets<R::RenderAsset>>() {
            app.add_resource(RenderAssets::<R::RenderAsset>::new());
        }

        self
    }

    fn extract_render_resource<R: RenderResource>(&mut self) -> &mut Self {
        self.get_or_insert_resource(|| ResourceExtractors::default())
            .add::<R>();
        self
    }
}
