use crate::{
    core::{RenderDevice, RenderSettings, RenderSurface, RenderSurfaceTexture, SurfaceResized},
    output::OutputPassPlugin,
    renderer::{
        camera::{Camera, CameraQueue},
        graph::{RenderGraph, RenderGraphDirty},
    },
    resources::{
        BindGroupLayoutRegistry, ExtractError, ExtractInfo, GpuMesh, GpuShader, GpuTexture,
        MainRenderTarget, PipelineCache, RenderAsset, RenderAssetRegistry, RenderAssets,
        RenderTarget, SamplerCache, ShaderVariants,
        allocator::{MeshAllocator, MeshAllocatorConfig},
    },
};
use asset::plugin::{AssetAppExt, AssetPlugin};
use ecs::{
    AppBuilder, AppTag, Extract, IntoSystemConfigs, Phase, Plugin, Run,
    app::sync::SyncComponentPlugin,
};
use std::marker::PhantomData;
use transform::GlobalTransform;
use window::{Window, plugin::WindowPlugin};

pub struct RenderPlugin;

#[derive(Default, Clone, Copy, AppTag)]
pub struct RenderApp;

impl Plugin for RenderPlugin {
    fn setup(&mut self, app: &mut AppBuilder) {
        app.add_plugins((
            WindowPlugin,
            AssetPlugin,
            OutputPassPlugin,
            RenderAssetPlugin::<GpuShader>::new(),
            RenderAssetPlugin::<GpuTexture>::new(),
            RenderAssetPlugin::<GpuMesh>::new(),
            RenderAssetPlugin::<RenderTarget>::new(),
            SyncComponentPlugin::<Camera, RenderApp>::new(),
        ))
        .register::<GlobalTransform>()
        .sub_app_mut(RenderApp)
        .add_sub_phase(Run, Process)
        .add_sub_phase(Run, Queue)
        .add_sub_phase(Run, Commit)
        .add_sub_phase(Run, PreRender)
        .add_sub_phase(Run, Render)
        .add_sub_phase(Run, Present)
        .add_resource(RenderGraph::default())
        .add_resource(RenderSettings::default())
        .add_resource(CameraQueue::default())
        .add_resource(PipelineCache::default())
        .add_resource(ShaderVariants::default())
        .add_resource(MeshAllocatorConfig::default())
        .add_resource(RenderSurfaceTexture::default())
        .add_resource(MainRenderTarget::default())
        .add_resource(BindGroupLayoutRegistry::default())
        .add_resource(SurfaceResized::default())
        .add_systems(Extract, SurfaceResized::extract)
        .add_systems(Process, RenderSurface::on_resized)
        .add_systems(Process, RenderSurfaceTexture::update)
        .add_systems(Process, MainRenderTarget::update)
        .add_systems(Queue, CameraQueue::queue)
        .add_systems(PreRender, RenderGraph::update.when::<RenderGraphDirty>())
        .add_systems(Render, RenderGraph::run)
        .add_systems(Present, RenderSurfaceTexture::present);
    }

    fn build(&mut self, app: &mut AppBuilder) {
        let window = app.resource::<Window>();
        let task = async {
            let mut surface = RenderSurface::new(window).await.unwrap();
            let device = RenderDevice::new(surface.adapter()).await.unwrap();

            surface.configure(&device);

            (surface, device)
        };

        let (surface, device) = smol::block_on(task);
        let samplers = SamplerCache::new(&device);

        app.sub_app_mut(RenderApp)
            .add_resource(surface)
            .add_resource(device)
            .add_resource(samplers);
    }

    fn finish(&mut self, app: &mut AppBuilder) {
        let app = app.sub_app_mut(RenderApp);

        let config = app
            .remove_resource::<MeshAllocatorConfig>()
            .unwrap_or_default();
        app.add_resource(MeshAllocator::new(config));

        if let Some(registry) = app.remove_resource::<RenderAssetRegistry>() {
            for config in registry.build() {
                app.add_systems(Extract, config.extract);
                app.add_systems(Process, config.process);
            }
        }

        app.add_systems(Extract, ShaderVariants::extract);
        app.add_systems(Process, PipelineCache::process);
        app.add_systems(Commit, MeshAllocator::commit);
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
pub struct PreRender;

#[derive(Phase)]
pub struct Render;

#[derive(Phase)]
pub struct Present;
