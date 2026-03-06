use crate::{
    core::{RenderDevice, RenderSurface},
    resources::{
        ExtractError, ExtractInfo, GpuShader, GpuTexture, PipelineCache, RenderAsset,
        RenderAssetRegistry, RenderAssets, ShaderVariants,
    },
};
use asset::plugin::{AssetAppExt, AssetPlugin};
use ecs::{AppBuilder, AppTag, Extract, Phase, Plugin, Run, app::sync::SyncEventsPlugin};
use std::marker::PhantomData;
use transform::GlobalTransform;
use wgpu::TextureFormat;
use window::{Window, events::WindowResized, plugin::WindowPlugin};

pub struct RenderPlugin;

#[derive(Default, Clone, Copy, AppTag)]
pub struct RenderApp;

impl Plugin for RenderPlugin {
    fn setup(&mut self, app: &mut AppBuilder) {
        app.add_plugins((
            WindowPlugin,
            AssetPlugin,
            RenderAssetPlugin::<GpuShader>::new(),
            RenderAssetPlugin::<GpuTexture>::new(),
            SyncEventsPlugin::<WindowResized, RenderApp>::new(),
        ))
        .register::<GlobalTransform>()
        .sub_app_mut(RenderApp)
        .add_sub_phase(Run, Process)
        .add_sub_phase(Run, Queue)
        .add_sub_phase(Run, Commit)
        .add_sub_phase(Run, Render)
        .add_sub_phase(Run, Present)
        .add_resource(PipelineCache::new())
        .add_resource(ShaderVariants::new());
    }

    fn build(&mut self, app: &mut AppBuilder) {
        let window = app.resource::<Window>();
        let task = async {
            let depth = TextureFormat::Depth32Float;
            let mut surface = RenderSurface::new(window, depth).await.unwrap();
            let device = RenderDevice::new(surface.adapter()).await.unwrap();

            surface.configure(&device);

            (surface, device)
        };

        let (surface, device) = smol::block_on(task);

        app.sub_app_mut(RenderApp)
            .add_resource(surface)
            .add_resource(device);
    }

    fn finish(&mut self, app: &mut AppBuilder) {
        let app = app.sub_app_mut(RenderApp);

        if let Some(registry) = app.remove_resource::<RenderAssetRegistry>() {
            for config in registry.build() {
                app.add_systems(Extract, config.extract);
                app.add_systems(Process, config.process);
            }
        }

        app.add_systems(Extract, ShaderVariants::extract);
        app.add_systems(Process, PipelineCache::process);
    }
}

pub struct RenderAssetPlugin<R: RenderAsset>(PhantomData<R>);
impl<R: RenderAsset> RenderAssetPlugin<R> {
    pub fn new() -> Self {
        Self(PhantomData)
    }
}

impl<R: RenderAsset> Plugin for RenderAssetPlugin<R> {
    fn setup(&mut self, app: &mut ecs::AppBuilder) {
        app.register_asset::<R::Asset>();

        let app = app.sub_app_mut(RenderApp);
        let registry =
            app.get_or_insert_resource::<RenderAssetRegistry>(|| RenderAssetRegistry::new());

        registry.register::<R>();
        R::dependencies(registry);

        app.add_resource(RenderAssets::<R>::default());
        app.add_resource(ExtractInfo::<R>::new());
        app.register_event::<ExtractError<R>>();
    }
}

#[derive(Phase)]
pub struct Process;

#[derive(Phase)]
pub struct Queue;

#[derive(Phase)]
pub struct Commit;

#[derive(Phase)]
pub struct Render;

#[derive(Phase)]
pub struct Present;
