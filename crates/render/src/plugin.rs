use std::fmt::format;

use crate::{
    Camera, CameraRenderTargets, CameraSubGraph, ExtractError, GlobalShaderConstant,
    GlobalShaderConstants, GpuShader, GpuTexture, ObjImporter, ProcessAssets, QueueDraws,
    QueueViews, RenderDevice, RenderGraphBuilder, RenderMesh, RenderPhase, RenderTarget,
    ShaderData, ShaderSettings, SubMesh, Texture2dImporter,
    allocator::MeshAllocatorPlugin,
    app::{PostRender, PreRender, Present, Process, Queue, Render, RenderApp},
    constants::StorageBufferEnabled,
    cpu::UniformDataBuffer,
    draw::{
        material::{Material, MaterialLayout},
        view::View,
    },
    drawable::{DrawPipeline, DrawSet, Drawable, ViewDrawSet},
    gpu::{
        CULLING_SHADER, DrawArgsBuffer, FrustumBuffer, GpuDrawResources, RenderEntityBuffer,
        StorageDataBuffer,
    },
    material::MaterialInstance,
    pass::{DrawPass, Renderer},
    processor::{ShaderConstant, ShaderConstants},
    renderer::{GraphPass, RenderGraph, SubGraph},
    resources::{
        AssetExtractors, ExtractInfo, Fallbacks, Mesh, PipelineCache, RenderAsset, RenderAssets,
        RenderResource, ResourceExtractors, Shader,
    },
    surface::{RenderSurface, RenderSurfaceTexture},
    view::{ViewBuffer, ViewSet},
};
use asset::{
    AssetId, embed_asset_with_path,
    io::AppAssets,
    plugin::{AssetAppExt, AssetPlugin},
};
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
            .add_systems(Init, RenderGraph::create_graph)
            .add_systems(Init, GlobalShaderConstants::init)
            .add_systems(Extract, RenderSurface::resize_surface)
            .add_systems(Queue, RenderSurface::queue_surface)
            .add_systems(Render, RenderGraph::run_graph)
            .add_systems(Present, RenderSurface::present_surface)
            .add_resource(RenderGraphBuilder::new())
            .add_resource(RenderSurfaceTexture::new())
            .add_resource(AssetExtractors::default())
            .add_resource(ResourceExtractors::default())
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
        let enabled = StorageBufferEnabled::get(&device);

        app.sub_app_mut(RenderApp)
            .add_resource(surface)
            .add_resource(device)
            .resource_mut::<GlobalShaderConstants>()
            .set(StorageBufferEnabled::NAME, enabled);
    }

    fn finish(&mut self, app: &mut AppBuilder) {
        if !app.resources().contains::<ResourceExtractors>() {
            app.add_resource(ResourceExtractors::default());
        }

        let app = app.sub_app_mut(RenderApp);
        app.add_systems(Extract, ResourceExtractors::extract);
        app.add_systems(Process, ResourceExtractors::process);

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
    fn add_pass<P: GraphPass>(&mut self, pass: P) -> &mut Self;
    fn add_sub_graph<S: SubGraph>(&mut self) -> &mut Self;
    fn add_sub_graph_pass<S: SubGraph, P: GraphPass>(&mut self, pass: P) -> &mut Self;
    fn add_renderer<R: Renderer>(&mut self) -> &mut Self;
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

    fn add_pass<P: GraphPass>(&mut self, pass: P) -> &mut Self {
        self.scoped_sub_app(RenderApp, |render_app| {
            render_app
                .get_or_insert_resource(RenderGraphBuilder::new)
                .add_pass(pass);
        })
    }

    fn add_sub_graph<S: SubGraph>(&mut self) -> &mut Self {
        self.scoped_sub_app(RenderApp, |render_app| {
            render_app
                .get_or_insert_resource(RenderGraphBuilder::new)
                .add_sub_graph::<S>();
        })
    }

    fn add_sub_graph_pass<S: SubGraph, P: GraphPass>(&mut self, pass: P) -> &mut Self {
        self.scoped_sub_app(RenderApp, |render_app| {
            render_app
                .get_or_insert_resource(RenderGraphBuilder::new)
                .add_sub_graph_pass::<S, P>(pass);
        })
    }

    fn add_renderer<R: Renderer>(&mut self) -> &mut Self {
        self.scoped_sub_app(RenderApp, |render_app| {
            render_app
                .get_or_insert_resource(RenderGraphBuilder::new)
                .add_sub_graph_pass::<CameraSubGraph, DrawPass<R>>(DrawPass::default());
        });

        self.add_plugins(CameraPlugin)
    }

    fn add_render_asset<R: RenderAsset>(&mut self) -> &mut Self {
        let app = self.sub_app_mut(RenderApp);
        app.resource_mut::<AssetExtractors>().add::<R>();

        if !app.resources().contains::<RenderAssets<R>>() {
            app.add_resource(RenderAssets::<R>::new());
        }

        if !app.resources().contains::<ExtractInfo<R>>() {
            app.add_resource(ExtractInfo::<R>::new());
        }

        self.register_event::<ExtractError<R>>();
        self.register_asset::<R::Source>()
    }

    fn add_render_resource<R: RenderResource>(&mut self) -> &mut Self {
        self.get_or_insert_resource(|| ResourceExtractors::default())
            .add::<R>();
        self.register_event::<ExtractError<R>>();
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
        .add_resource(CameraRenderTargets::default())
        .add_systems(PreRender, CameraRenderTargets::queue)
        .add_systems(PostRender, CameraRenderTargets::cleanup);
    }
}

pub struct ViewPlugin<V: View>(std::marker::PhantomData<V>);
impl<V: View> ViewPlugin<V> {
    pub fn new() -> Self {
        Self(Default::default())
    }
}
impl<V: View> Plugin for ViewPlugin<V> {
    fn setup(&mut self, app: &mut ecs::AppBuilder) {
        app.add_plugins((CameraPlugin, TransformPlugin::<V::Transform>::new()))
            .register::<V>()
            .add_render_resource::<ViewBuffer<V>>()
            .sub_app_mut(RenderApp)
            .add_resource(ViewSet::<V>::new())
            .add_systems(Extract, ViewSet::<V>::extract)
            .add_systems(Queue, ViewBuffer::<V>::queue)
            .add_systems(PostRender, ViewBuffer::<V>::reset_buffer);
    }

    fn finish(&mut self, app: &mut AppBuilder) {
        if let Some(ShaderConstant::Bool(true)) = app
            .sub_app_mut(RenderApp)
            .resource::<GlobalShaderConstants>()
            .get(StorageBufferEnabled::NAME)
        {
            app.add_render_resource::<FrustumBuffer>()
                .sub_app_mut(RenderApp)
                .add_systems(
                    Extract,
                    FrustumBuffer::extract::<V>.when::<Exists<FrustumBuffer>>(),
                );
        }
    }
}

struct GpuDrawPlugin;
impl Plugin for GpuDrawPlugin {
    fn setup(&mut self, app: &mut AppBuilder) {
        app.add_render_resource::<FrustumBuffer>()
            .add_render_resource::<RenderEntityBuffer>()
            .add_render_resource::<DrawArgsBuffer>();
    }
}

pub struct ModelPlugin<T: ShaderData>(std::marker::PhantomData<T>);
impl<T: ShaderData> ModelPlugin<T> {
    pub fn new() -> Self {
        Self(Default::default())
    }
}
impl<T: ShaderData> Plugin for ModelPlugin<T> {
    fn setup(&mut self, _: &mut ecs::AppBuilder) {}

    fn finish(&mut self, app: &mut AppBuilder) {
        if let Some(ShaderConstant::Bool(true)) = app
            .sub_app_mut(RenderApp)
            .resource::<GlobalShaderConstants>()
            .get(StorageBufferEnabled::NAME)
        {
            app.add_plugins(GpuDrawPlugin)
                .add_render_resource::<StorageDataBuffer<T>>()
                .add_render_resource::<GpuDrawResources<T>>();

            let assets = app.get_or_insert_resource(|| AppAssets::<RenderApp>::default());
            let name = ecs::ext::short_type_name::<T>();
            let id = CULLING_SHADER.with_namespace(name.as_bytes());
            let path = format!("embedded/shaders/culling.{}.wgsl", name);
            let def = ShaderConstant::Str(generate_padded_struct(
                "InstanceData",
                std::mem::size_of::<T>(),
            ));
            let mut constants = ShaderConstants::new();
            constants.set("INSTANCE_DATA", def);

            embed_asset_with_path!(
                assets,
                id,
                "embedded/shaders/culling.wgsl",
                &path,
                ShaderSettings::from(constants)
            );
        } else {
            app.add_render_resource::<UniformDataBuffer<T>>()
                .sub_app_mut(RenderApp)
                .add_systems(PreRender, UniformDataBuffer::<T>::update_buffer)
                .add_systems(PostRender, UniformDataBuffer::<T>::clear_buffer);
        }
    }
}

pub struct RenderPhasePlugin<P: RenderPhase>(std::marker::PhantomData<P>);
impl<P: RenderPhase> RenderPhasePlugin<P> {
    pub fn new() -> Self {
        Self(Default::default())
    }
}
impl<P: RenderPhase> Plugin for RenderPhasePlugin<P> {
    fn setup(&mut self, app: &mut AppBuilder) {
        app.add_plugins(ViewPlugin::<P::View>::new())
            .add_resource(ViewDrawSet::<P::View, P>::new())
            .sub_app_mut(RenderApp)
            .add_systems(PostRender, ViewDrawSet::<P::View, P>::clear_set);
    }
}

pub struct MaterialPlugin<M: Material>(std::marker::PhantomData<M>);
impl<M: Material> MaterialPlugin<M> {
    pub fn new() -> Self {
        Self(Default::default())
    }
}
impl<M: Material> Plugin for MaterialPlugin<M> {
    fn setup(&mut self, app: &mut ecs::AppBuilder) {
        app.add_plugins((RenderPlugin, RenderPhasePlugin::<M::Phase>::new()))
            .register_asset::<M>()
            .register_resource::<MaterialLayout<M>>()
            .add_render_asset::<MaterialInstance<M>>();
    }
}

pub struct DrawPlugin<D: Drawable>(std::marker::PhantomData<D>);
impl<D: Drawable> DrawPlugin<D> {
    pub fn new() -> Self {
        Self(Default::default())
    }
}
impl<D: Drawable> Plugin for DrawPlugin<D> {
    fn setup(&mut self, app: &mut ecs::AppBuilder) {
        app.add_plugins((
            ModelPlugin::<D::Model>::new(),
            MaterialPlugin::<D::Material>::new(),
        ))
        .scoped_sub_app(RenderApp, |app| {
            app.add_resource(DrawSet::<D>::new())
                .add_resource(ViewDrawSet::<D::View, <D::Material as Material>::Phase>::new())
                .add_systems(Extract, DrawSet::<D>::extract)
                .add_systems(PostRender, DrawSet::<D>::clear_set);
        })
        .register::<D>()
        .add_render_resource::<DrawPipeline<D>>()
        .load_asset::<GpuShader>(D::shader().into())
        .load_asset::<GpuShader>(D::Material::shader().into());
    }

    fn finish(&mut self, app: &mut AppBuilder) {
        let app = app.sub_app_mut(RenderApp);

        if let Some(ShaderConstant::Bool(true)) = app
            .resource::<GlobalShaderConstants>()
            .get(StorageBufferEnabled::NAME)
        {
            app.add_systems(
                QueueDraws,
                StorageDataBuffer::queue::<D, <D::Material as Material>::Phase>
                    .when::<Exists<DrawPipeline<D>>>(),
            );
        } else {
            let batch_size =
                UniformDataBuffer::<D::Model>::get_batch_size(app.resource::<RenderDevice>());
            let mut constants = ShaderConstants::new();
            constants.set("BATCH_SIZE", ShaderConstant::U32(batch_size));

            app.resource_mut::<GlobalShaderConstants>()
                .add_local(D::shader().into(), constants);

            app.add_systems(
                QueueDraws,
                UniformDataBuffer::<D::Model>::queue::<D, <D::Material as Material>::Phase>
                    .when::<Exists<DrawPipeline<D>>>(),
            );
        }
    }
}

fn generate_padded_struct(name: &str, size_in_bytes: usize) -> String {
    let padded_size = (size_in_bytes + 15) & !15; // round up to nearest multiple of 16
    let full_chunks = padded_size / 16;

    let mut struct_def = format!("struct {name} {{\n",);

    for i in 0..full_chunks {
        struct_def.push_str(&format!("    data{}: vec4<u32>;\n", i));
    }

    struct_def.push_str("};");
    struct_def
}
