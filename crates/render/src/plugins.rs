use crate::{
    allocator::MeshAllocatorPlugin, app::{PostRender, PreRender, Present, Process, Queue, Render, RenderApp}, draw::{
        BlendMode, Draw, DrawArgs, DrawCalls, DrawCommands, DrawModel, DrawPhase, DrawPipeline, Drawable, Material, MaterialInstance, MaterialLayout, ObjectBuffer, OpaquePhase, RenderPhase, ShaderModel, TransparentPhase, View, ViewBuffer, ViewInstance
    }, resources::{
        AssetExtractors, ExtractInfo, Fallbacks, Mesh, PipelineCache, RenderAsset, RenderAssets,
        RenderResource, ResourceExtractors, Shader,
    }, surface::{RenderSurface, RenderSurfaceTexture}, ActiveCamera, Camera, CameraAttachments, CameraPhase, CameraSortOrder, ExtractError, GlobalShaderConstant, GlobalShaderConstants, GpuShader, GpuTexture, MeshFilter, ObjImporter, ProcessAssets, QueueDraws, QueueViews, RenderDevice, RenderMesh, RenderTarget, SubMesh, Texture2dImporter
};
use asset::plugin::{AssetAppExt, AssetPlugin};
use ecs::{
    AppBuilder, Extract, Init, IntoSystemConfig, Phase, Plugin, Run, app::sync::SyncComponentPlugin,
};
use std::marker::PhantomData;
use transform::{GlobalTransform, plugin::TransformPlugin};
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

        app.sub_app_mut(RenderApp)
            .add_resource(surface)
            .add_resource(device);
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
    fn add_drawable<D: Drawable>(&mut self) -> &mut Self;
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
    fn add_drawable<D: Drawable>(&mut self) -> &mut Self {
        self.add_plugins(DrawPlugin::<D>::new())
    }

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

pub struct ViewPlugin<V: View>(PhantomData<V>);
impl<V: View> ViewPlugin<V> {
    pub fn new() -> Self {
        Self(PhantomData)
    }
}

impl<V: View> Plugin for ViewPlugin<V> {
    fn setup(&mut self, app: &mut ecs::AppBuilder) {
        app.add_plugins((
            CameraPlugin,
            RenderPlugin,
            TransformPlugin::<V::Transform>::new(),
            SyncComponentPlugin::<V, RenderApp>::new(),
            SyncComponentPlugin::<V::Transform, RenderApp>::new(),
            SyncComponentPlugin::<GlobalTransform, RenderApp>::new(),
        ))
        .sub_app_mut(RenderApp)
        .register::<ViewInstance<V>>()
        .add_systems(QueueViews, ViewBuffer::<V>::queue)
        .add_systems(PreRender, ViewBuffer::<V>::update)
        .add_systems(PostRender, ViewBuffer::<V>::clear);
    }

    fn build(&mut self, app: &mut ecs::AppBuilder) {
        let buffer = {
            let device = app.sub_app_mut(RenderApp).resource::<RenderDevice>();
            ViewBuffer::<V>::new(device)
        };

        app.sub_app_mut(RenderApp).add_resource(buffer);
    }
}

pub struct MaterialPlugin<M: Material>(PhantomData<M>);
impl<M: Material> MaterialPlugin<M> {
    pub fn new() -> Self {
        Self(PhantomData)
    }
}

impl<M: Material> Plugin for MaterialPlugin<M> {
    fn setup(&mut self, app: &mut ecs::AppBuilder) {
        app.add_plugins(RenderPlugin)
            .add_render_asset::<MaterialInstance<M>>();
    }

    fn build(&mut self, app: &mut ecs::AppBuilder) {
        let layout = {
            let device = app.sub_app_mut(RenderApp).resource::<RenderDevice>();
            MaterialLayout::<M>::new(device)
        };

        app.sub_app_mut(RenderApp).add_resource(layout);
    }
}

pub struct ShaderModelPlugin<M: ShaderModel>(PhantomData<M>);
impl<M: ShaderModel> ShaderModelPlugin<M> {
    pub fn new() -> Self {
        Self(PhantomData)
    }
}

impl<M: ShaderModel> Plugin for ShaderModelPlugin<M> {
    fn setup(&mut self, app: &mut ecs::AppBuilder) {
        app.add_plugins(RenderPlugin)
            .sub_app_mut(RenderApp)
            .add_systems(Init, |world: &mut ecs::World| {
                world.add_resource(M::create(world));
            });
    }
}

pub struct RenderPhasePlugin<P: RenderPhase>(PhantomData<P>);
impl<P: RenderPhase> RenderPhasePlugin<P> {
    pub fn new() -> Self {
        Self(PhantomData)
    }
}

impl<P: RenderPhase> Plugin for RenderPhasePlugin<P> {
    fn setup(&mut self, app: &mut AppBuilder) {
        app.add_plugins(ViewPlugin::<P::View>::new());

        let app = app.sub_app_mut(RenderApp);
        app.add_resource(DrawCalls::<P>::new())
            .add_systems(PostRender, DrawCalls::<P>::clear)
            .get_or_insert_resource(|| DrawCommands::default());

        match P::mode() {
            BlendMode::Opaque => {
                app.add_sub_phase(CameraPhase, OpaquePhase);
                app.add_systems(OpaquePhase, DrawCalls::<P>::draw);
            }
            BlendMode::Transparent => {
                app.add_sub_phase(CameraPhase, TransparentPhase);
                app.run_after(TransparentPhase, OpaquePhase);
                app.add_systems(TransparentPhase, DrawCalls::<P>::draw);
            }
        };
    }
}

pub struct ObjectBufferPlugin;
impl Plugin for ObjectBufferPlugin {
    fn setup(&mut self, app: &mut ecs::AppBuilder) {
        app.add_plugins(RenderPlugin)
            .sub_app_mut(RenderApp)
            .add_systems(PreRender, ObjectBuffer::update)
            .add_systems(PostRender, ObjectBuffer::clear)
            .add_systems(PreRender, DrawArgs::update)
            .add_systems(PostRender, DrawArgs::clear);
    }

    fn build(&mut self, app: &mut ecs::AppBuilder) {
        let (objects, args) = {
            let device = app.sub_app_mut(RenderApp).resource::<RenderDevice>();
            (ObjectBuffer::new(device), DrawArgs::new(device))
        };

        app.sub_app_mut(RenderApp)
            .add_resource(objects)
            .add_resource(args);
    }
}

pub struct DrawPlugin<D: Drawable>(PhantomData<D>);
impl<D: Drawable> DrawPlugin<D> {
    pub fn new() -> Self {
        Self(PhantomData)
    }
}

impl<D: Drawable> Plugin for DrawPlugin<D> {
    fn setup(&mut self, app: &mut ecs::AppBuilder) {
        let app = app
            .add_plugins((
                ViewPlugin::<D::View>::new(),
                ObjectBufferPlugin,
                MaterialPlugin::<D::Material>::new(),
                RenderPhasePlugin::<DrawPhase<D>>::new(),
                ShaderModelPlugin::<DrawModel<D>>::new(),
                SyncComponentPlugin::<D, RenderApp>::new(),
                SyncComponentPlugin::<MeshFilter, RenderApp>::new(),
            ))
            .load_asset::<Shader>(D::shader().into())
            .load_asset::<Shader>(D::Material::shader().into())
            .sub_app_mut(RenderApp);

        let mut commands = app.remove_resource::<DrawCommands>().unwrap_or_default();
        commands.add::<Draw<D>>(app.world_mut());
        app.add_resource(commands)
            .add_systems(Init, DrawPipeline::<D>::queue)
            .add_systems(QueueDraws, DrawCalls::<DrawPhase<D>>::queue::<D>);
    }
}
