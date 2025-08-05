use crate::{
    ActiveCamera, Camera, CameraAttachments, CameraPhase, CameraSortOrder, ExtractError,
    GlobalShaderConstant, GlobalShaderConstants, GpuShader, GpuTexture, MeshFilter, ObjImporter,
    ProcessAssets, QueueDraws, QueueViews, RenderDevice, RenderMesh, RenderTarget, SubMesh,
    Texture2dImporter,
    allocator::MeshAllocatorPlugin,
    app::{PostRender, PreRender, Present, Process, Queue, Render, RenderApp},
    constants::{StorageBufferEnabled, UniformBatchSize},
    draw::{
        BatchedUniformBuffer, DrawModel, DrawPass, DrawPhase, DrawPipeline, Drawable, MainDrawPass,
        Material, MaterialInstance, MaterialLayout, ModelData, ModelUniformData, PhaseDrawCalls,
        ShaderModel, ShaderModelData, ShaderPhase, ShaderPhases, View, ViewBuffer,
    },
    processor::{ShaderConstant, ShaderConstants},
    resources::{
        AssetExtractors, ExtractInfo, Fallbacks, Mesh, PipelineCache, RenderAsset, RenderAssets,
        RenderResource, ResourceExtractors, Shader,
    },
    surface::{RenderSurface, RenderSurfaceTexture},
};
use asset::plugin::{AssetAppExt, AssetPlugin};
use ecs::{
    AppBuilder, Extract, Init, IntoSystemConfig, Plugin, Run, app::sync::SyncComponentPlugin,
    system::Exists,
};
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
    fn add_drawable<D: Drawable>(&mut self) -> &mut Self;
    fn add_shader_constant<C: GlobalShaderConstant>(&mut self) -> &mut Self;
    // fn add_pass<P: GraphPass>(&mut self, pass: P) -> &mut Self;
    // fn add_sub_graph<S: SubGraph>(&mut self) -> &mut Self;
    // fn add_sub_graph_pass<S: SubGraph, P: GraphPass>(&mut self, pass: P) -> &mut Self;
    // fn add_nested_sub_graph<S: SubGraph, Nested: SubGraph>(&mut self) -> &mut Self;
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

    // fn add_pass<P: GraphPass>(&mut self, pass: P) -> &mut Self {
    //     self.scoped_sub_app(RenderApp, |render_app| {
    //         render_app
    //             .get_or_insert_resource(RenderGraphBuilder::new)
    //             .add_pass(pass);
    //     })
    // }

    // fn add_sub_graph<S: SubGraph>(&mut self) -> &mut Self {
    //     self.scoped_sub_app(RenderApp, |render_app| {
    //         render_app
    //             .get_or_insert_resource(RenderGraphBuilder::new)
    //             .add_sub_graph::<S>();
    //     })
    // }

    // fn add_sub_graph_pass<S: SubGraph, P: GraphPass>(&mut self, pass: P) -> &mut Self {
    //     self.scoped_sub_app(RenderApp, |render_app| {
    //         render_app
    //             .get_or_insert_resource(RenderGraphBuilder::new)
    //             .add_sub_graph_pass::<S, P>(pass);
    //     })
    // }

    // fn add_nested_sub_graph<S: SubGraph, Nested: SubGraph>(&mut self) -> &mut Self {
    //     self.scoped_sub_app(RenderApp, |render_app| {
    //         render_app
    //             .get_or_insert_resource(RenderGraphBuilder::new)
    //             .add_nested_sub_graph::<S, Nested>();
    //     })
    // }

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

pub struct ViewPlugin<V: View>(std::marker::PhantomData<V>);
impl<V: View> ViewPlugin<V> {
    pub fn new() -> Self {
        Self(std::marker::PhantomData)
    }
}
impl<V: View> Plugin for ViewPlugin<V> {
    fn setup(&mut self, app: &mut AppBuilder) {
        app.add_plugins((
            SyncComponentPlugin::<V, RenderApp>::new(),
            SyncComponentPlugin::<V::Transform, RenderApp>::new(),
            SyncComponentPlugin::<GlobalTransform, RenderApp>::new(),
            TransformPlugin::<V::Transform>::new(),
            CameraPlugin,
        ))
        .add_render_resource::<ViewBuffer<V>>()
        .sub_app_mut(RenderApp)
        .add_systems(QueueViews, ViewBuffer::<V>::queue);
    }
}

pub struct ModelDataPlugin<T: ModelUniformData>(std::marker::PhantomData<T>);
impl<T: ModelUniformData> ModelDataPlugin<T> {
    pub fn new() -> Self {
        Self(std::marker::PhantomData)
    }
}
impl<T: ModelUniformData> Plugin for ModelDataPlugin<T> {
    fn setup(&mut self, app: &mut AppBuilder) {
        app.add_plugins(RenderPlugin)
            .add_render_resource::<BatchedUniformBuffer<T>>()
            .sub_app_mut(RenderApp)
            .add_systems(PreRender, BatchedUniformBuffer::<T>::update_buffer)
            .add_systems(PostRender, BatchedUniformBuffer::<T>::reset_buffer);
    }
}

pub struct MaterialPlugin<M: Material>(std::marker::PhantomData<M>);
impl<M: Material> MaterialPlugin<M> {
    pub fn new() -> Self {
        Self(std::marker::PhantomData)
    }
}
impl<M: Material> Plugin for MaterialPlugin<M> {
    fn setup(&mut self, app: &mut AppBuilder) {
        app.add_plugins(RenderPlugin)
            .register_asset::<M>()
            .add_render_asset::<MaterialInstance<M>>()
            .add_render_resource::<MaterialLayout<M>>();
    }
}

pub struct ShaderModelPlugin<M: ShaderModel>(std::marker::PhantomData<M>);
impl<M: ShaderModel> ShaderModelPlugin<M> {
    pub fn new() -> Self {
        Self(std::marker::PhantomData)
    }
}

impl<M: ShaderModel> Plugin for ShaderModelPlugin<M> {
    fn setup(&mut self, app: &mut AppBuilder) {
        app.add_plugins(CameraPlugin)
            .sub_app_mut(RenderApp)
            .add_sub_phase(CameraPhase, MainDrawPass)
            .add_sub_phase(MainDrawPass, DrawPass::<M>::new());

        let mut phases = ShaderPhases::new();
        M::setup(&mut phases);

        for v in phases.0.chunks_exact_mut(2) {
            v[1].add_dependency(v[0].id());
        }

        app.sub_app_mut(RenderApp)
            .add_systems(DrawPass::<M>::new(), phases.0);
    }
}

pub struct ShaderPhasePlugin<P: ShaderPhase, M: ShaderModel>(std::marker::PhantomData<(P, M)>);
impl<P: ShaderPhase, M: ShaderModel> ShaderPhasePlugin<P, M> {
    pub fn new() -> Self {
        Self(std::marker::PhantomData)
    }
}

impl<P: ShaderPhase, M: ShaderModel> Plugin for ShaderPhasePlugin<P, M> {
    fn setup(&mut self, app: &mut AppBuilder) {
        app.add_plugins(ShaderModelPlugin::<M>::new())
            .add_render_resource::<ShaderModelData<M>>()
            .sub_app_mut(RenderApp)
            .add_resource(PhaseDrawCalls::<P, M>::new())
            .add_systems(PostRender, PhaseDrawCalls::<P, M>::clear);
    }
}

pub struct DrawPlugin<D: Drawable>(std::marker::PhantomData<D>);
impl<D: Drawable> DrawPlugin<D> {
    pub fn new() -> Self {
        Self(std::marker::PhantomData)
    }
}

impl<D: Drawable> Plugin for DrawPlugin<D> {
    fn setup(&mut self, app: &mut AppBuilder) {
        app.add_plugins((
            ViewPlugin::<D::View>::new(),
            ModelDataPlugin::<ModelData>::new(),
            MaterialPlugin::<D::Material>::new(),
            ShaderPhasePlugin::<DrawPhase<D>, DrawModel<D>>::new(),
            SyncComponentPlugin::<D, RenderApp>::new(),
            SyncComponentPlugin::<MeshFilter, RenderApp>::new(),
        ))
        .load_asset::<Shader>(D::shader().into())
        .load_asset::<Shader>(D::Material::shader().into())
        .add_render_resource::<DrawPipeline<D>>()
        .sub_app_mut(RenderApp)
        .add_systems(
            QueueDraws,
            BatchedUniformBuffer::<ModelData>::queue::<D>.when::<Exists<DrawPipeline<D>>>(),
        );
    }

    fn build(&mut self, app: &mut AppBuilder) {
        let constants = {
            let device = app.sub_app_mut(RenderApp).resource::<RenderDevice>();

            let mut constants = ShaderConstants::new();
            constants.set(
                UniformBatchSize::<ModelData>::NAME,
                UniformBatchSize::<ModelData>::get(device),
            );

            constants
        };

        app.sub_app_mut(RenderApp)
            .resource_mut::<GlobalShaderConstants>()
            .add_local(D::shader().into(), constants);
    }
}
