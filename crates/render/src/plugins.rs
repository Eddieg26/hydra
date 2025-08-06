use crate::{
    ActiveCamera, Camera, CameraAttachments, CameraPhase, CameraSortOrder, ExtractError,
    GlobalShaderConstant, GlobalShaderConstants, GpuShader, GpuTexture, ObjImporter, ProcessAssets,
    QueueDraws, QueueViews, RenderDevice, RenderMesh, RenderTarget, SubMesh, Texture2dImporter,
    allocator::MeshAllocatorPlugin,
    app::{PostRender, PreRender, Present, Process, Queue, Render, RenderApp},
    constants::StorageBufferEnabled,
    processor::ShaderConstant,
    resources::{
        AssetExtractors, ExtractInfo, Fallbacks, Mesh, PipelineCache, RenderAsset, RenderAssets,
        RenderResource, ResourceExtractors, Shader,
    },
    surface::{RenderSurface, RenderSurfaceTexture},
};
use asset::plugin::{AssetAppExt, AssetPlugin};
use ecs::{
    AppBuilder, Extract, Init, IntoSystemConfig, Phase, Plugin, Run, app::sync::SyncComponentPlugin,
};
use transform::GlobalTransform;
use window::{Window, plugin::WindowPlugin};

pub struct RenderPlugin;

impl Plugin for RenderPlugin {
    fn setup(&mut self, app: &mut ecs::AppBuilder) {
        app.add_plugins((WindowPlugin, AssetPlugin))
            .register::<GlobalTransform>()
            .sub_app_mut(RenderApp)
            .add_plugins(MeshAllocatorPlugin)
            .add_sub_phase(Run, Process)
            .add_sub_phase(Process, ProcessAssets)
            .add_sub_phase(Run, Queue)
            .add_sub_phase(Queue, QueueViews)
            .add_sub_phase(Queue, QueueDraws)
            .add_sub_phase(Run, PreRender)
            .add_sub_phase(Run, Render)
            .add_sub_phase(Run, Present)
            .add_sub_phase(Run, PostRender)
            .add_systems(Init, GlobalShaderConstants::init)
            .add_systems(Extract, RenderSurface::resize_surface)
            .add_systems(Queue, RenderSurface::queue_surface)
            .add_systems(Present, RenderSurface::present_surface)
            .add_resource(RenderSurfaceTexture::new())
            .add_resource(PipelineCache::default())
            .add_resource(GlobalShaderConstants::new())
            .register_event::<ExtractError>();

        app.add_render_resource::<Fallbacks>()
            .add_render_asset::<GpuShader>()
            .add_render_asset::<RenderMesh>()
            .add_render_asset::<GpuTexture>()
            .add_render_asset::<RenderTarget>()
            .add_render_asset::<SubMesh>()
            .add_importer::<Shader>()
            .add_importer::<ObjImporter>()
            .add_importer::<Texture2dImporter>()
            .add_loader::<SubMesh>()
            .set_default_processor::<Mesh>()
            .set_default_processor::<Shader>();
    }

    fn build(&mut self, app: &mut AppBuilder) {
        let window = app.resource::<Window>();
        let task = async {
            let (surface, adapter) = RenderSurface::new(window).await.unwrap();
            let device = RenderDevice::new(&adapter).await.unwrap();

            surface.configure(&device);

            (surface, device)
        };

        let (surface, device) = smol::block_on(task);
        // TODO: let enabled = StorageBufferEnabled::get(&device);
        let enabled = ShaderConstant::Bool(false);

        app.sub_app_mut(RenderApp)
            .add_resource(surface)
            .add_resource(device)
            .resource_mut::<GlobalShaderConstants>()
            .set(StorageBufferEnabled::NAME, enabled);
    }

    fn finish(&mut self, app: &mut AppBuilder) {
        let app = app.sub_app_mut(RenderApp);

        let configs = app
            .remove_resource::<ResourceExtractors>()
            .map(|e| e.0.into_values().collect::<Vec<_>>())
            .unwrap_or_default();

        app.add_systems(Process, configs);

        let configs = app
            .remove_resource::<AssetExtractors>()
            .map(|a| a.build())
            .unwrap_or_default();

        for config in configs {
            app.add_systems(Extract, config.extract);
            app.add_systems(ProcessAssets, config.process);
        }

        app.add_systems(Queue, PipelineCache::queue);
    }
}

pub trait RenderAppExt {
    fn add_shader_constant<C: GlobalShaderConstant>(&mut self) -> &mut Self;
    fn add_pass<M>(&mut self, pass: impl IntoSystemConfig<M>) -> &mut Self;
    fn add_sub_pass<M>(
        &mut self,
        phase: impl Phase + Clone,
        pass: impl IntoSystemConfig<M>,
    ) -> &mut Self;
    fn add_render_asset<R: RenderAsset>(&mut self) -> &mut Self;
    fn add_render_resource<R: RenderResource>(&mut self) -> &mut Self;
}

impl RenderAppExt for AppBuilder {
    fn add_shader_constant<C: GlobalShaderConstant>(&mut self) -> &mut Self {
        self.get_or_insert_resource(|| GlobalShaderConstants::new())
            .register::<C>();
        self
    }

    fn add_pass<M>(&mut self, pass: impl IntoSystemConfig<M>) -> &mut Self {
        self.scoped_sub_app(RenderApp, move |app| {
            app.add_systems(Render, pass);
        })
    }

    fn add_sub_pass<M>(
        &mut self,
        phase: impl Phase + Clone,
        pass: impl IntoSystemConfig<M>,
    ) -> &mut Self {
        self.scoped_sub_app(RenderApp, move |app| {
            app.add_sub_phase(Render, phase.clone());
            app.add_systems(phase, pass);
        })
    }

    fn add_render_asset<R: RenderAsset>(&mut self) -> &mut Self {
        let app = self.sub_app_mut(RenderApp);
        app.get_or_insert_resource(|| AssetExtractors::default())
            .add::<R>();

        if !app.resources().contains::<RenderAssets<R>>() {
            app.add_resource(RenderAssets::<R>::new());
        }

        if !app.resources().contains::<ExtractInfo<R>>() {
            app.add_resource(ExtractInfo::<R>::new());
        }

        app.register_event::<ExtractError<R>>();
        self.register_asset::<R::Source>()
    }

    fn add_render_resource<R: RenderResource>(&mut self) -> &mut Self {
        let app = self.sub_app_mut(RenderApp);
        app.get_or_insert_resource(|| ResourceExtractors::default())
            .add::<R>();

        app.register_event::<ExtractError<R>>();

        self
    }
}

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
