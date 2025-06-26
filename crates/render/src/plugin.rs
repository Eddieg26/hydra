use crate::{
    CameraSubGraph, DisableCulling, Draw, DrawPipeline, DrawTree, ExtractError, GpuTexture,
    MaterialBinding, ObjImporter, PostProcess, PreProcess, PreQueue, RenderMesh, RenderTarget,
    Shader, SubMesh, Texture2dImporter, ViewDrawCalls, VisibleDraws,
    app::{PostRender, PreRender, Present, Process, Queue, Render, RenderApp},
    draw::{
        Renderer, RendererPass,
        material::{Material, MaterialLayout, RenderPhase},
        surface::{
            BatchedMeshDataBuffer, MeshDataBuffer, ShaderData, clear_mesh_data_buffers,
            update_mesh_data_buffers,
        },
        view::{View, ViewDataBuffer},
    },
    renderer::{Camera, EntityCameras, RenderGraph, RenderGraphPass, SubGraph},
    resources::{
        AssetExtractors, ExtractInfo, Fallbacks, Mesh, PipelineCache, RenderAsset, RenderAssets,
        RenderResource, ResourceExtractors, ShaderSource,
    },
    surface::{RenderSurface, RenderSurfaceTexture},
};
use asset::plugin::{AssetAppExt, AssetPlugin};
use ecs::{AppBuilder, Extract, Init, IntoSystemConfig, Plugin, Run, system::Exists};
use transform::GlobalTransform;
use window::plugin::WindowPlugin;

pub struct RenderPlugin;

impl Plugin for RenderPlugin {
    fn setup(&mut self, app: &mut ecs::AppBuilder) {
        app.add_plugins((WindowPlugin, AssetPlugin))
            .add_sub_app(RenderApp)
            .add_sub_phase(Run, PreProcess)
            .add_sub_phase(Run, Process)
            .add_sub_phase(Run, PostProcess)
            .add_sub_phase(Run, PreQueue)
            .add_sub_phase(Run, Queue)
            .add_sub_phase(Run, PreRender)
            .add_sub_phase(Run, Render)
            .add_sub_phase(Run, PostRender)
            .add_sub_phase(Run, Present)
            .add_systems(Init, RenderSurface::create_surface)
            .add_systems(Extract, RenderSurface::resize_surface)
            .add_systems(Queue, RenderSurface::queue_surface)
            .add_systems(Render, RenderGraph::run_graph)
            .add_systems(Present, RenderSurface::present_surface)
            .add_systems(Extract, EntityCameras::extract)
            .add_resource(RenderGraph::new())
            .add_resource(RenderSurfaceTexture::new())
            .add_resource(EntityCameras::default())
            .add_resource(AssetExtractors::default())
            .add_resource(ResourceExtractors::default())
            .add_resource(PipelineCache::default())
            .register_event::<ExtractError>();

        app.register::<Camera>()
            .add_render_resource::<Fallbacks>()
            .add_render_asset::<RenderMesh>()
            .add_render_asset::<GpuTexture>()
            .add_render_asset::<RenderTarget>()
            .add_render_asset::<SubMesh>()
            .add_render_asset::<Shader>()
            .add_importer::<ShaderSource>()
            .add_importer::<ObjImporter>()
            .add_importer::<Texture2dImporter>()
            .add_loader::<SubMesh>()
            .set_default_processor::<Mesh>();
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
        }

        app.add_systems(Extract, ResourceExtractors::extract);
        app.add_systems(PreProcess, ResourceExtractors::process);
        app.add_systems(Process, PipelineCache::process);
    }
}

pub trait RenderAppExt {
    fn register_draw<D: Draw>(&mut self) -> &mut Self;
    fn add_pass<P: RenderGraphPass>(&mut self, pass: P) -> &mut Self;
    fn add_sub_graph<S: SubGraph>(&mut self) -> &mut Self;
    fn add_sub_graph_pass<S: SubGraph, P: RenderGraphPass>(&mut self, pass: P) -> &mut Self;
    fn add_renderer<R: Renderer>(&mut self) -> &mut Self;
    fn add_render_asset<R: RenderAsset>(&mut self) -> &mut Self;
    fn add_render_resource<R: RenderResource>(&mut self) -> &mut Self;
}

impl RenderAppExt for AppBuilder {
    fn register_draw<D: Draw>(&mut self) -> &mut Self {
        self.add_plugins(DrawPlugin::<D>::new())
    }

    fn add_pass<P: RenderGraphPass>(&mut self, pass: P) -> &mut Self {
        self.scoped_sub_app(RenderApp, |render_app| {
            render_app
                .get_or_insert_resource(RenderGraph::new)
                .add_pass(pass);
        })
    }

    fn add_sub_graph<S: SubGraph>(&mut self) -> &mut Self {
        self.scoped_sub_app(RenderApp, |render_app| {
            render_app
                .get_or_insert_resource(RenderGraph::new)
                .add_sub_graph::<S>();
        })
    }

    fn add_sub_graph_pass<S: SubGraph, P: RenderGraphPass>(&mut self, pass: P) -> &mut Self {
        self.scoped_sub_app(RenderApp, |render_app| {
            render_app
                .get_or_insert_resource(RenderGraph::new)
                .add_sub_graph_pass::<S, P>(pass);
        })
    }

    fn add_renderer<R: Renderer>(&mut self) -> &mut Self {
        self.scoped_sub_app(RenderApp, |render_app| {
            render_app
                .get_or_insert_resource(RenderGraph::new)
                .add_sub_graph_pass::<CameraSubGraph, RendererPass<R>>(RendererPass::default());
        });

        self
    }

    fn add_render_asset<R: RenderAsset>(&mut self) -> &mut Self {
        let app = self.sub_app_mut(RenderApp).unwrap();
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

pub struct MaterialPlugin<M: Material>(std::marker::PhantomData<M>);
impl<M: Material> MaterialPlugin<M> {
    pub fn new() -> Self {
        Self(Default::default())
    }
}

impl<M: Material> Plugin for MaterialPlugin<M> {
    fn setup(&mut self, app: &mut ecs::AppBuilder) {
        app.add_plugins(RenderPlugin)
            .register_asset::<M>()
            .register_resource::<MaterialLayout<M>>()
            .add_render_asset::<MaterialBinding<M>>();
    }
}

struct ViewPlugin<V: View>(std::marker::PhantomData<V>);
impl<V: View> ViewPlugin<V> {
    pub fn new() -> Self {
        Self(Default::default())
    }
}

impl<V: View> Plugin for ViewPlugin<V> {
    fn setup(&mut self, app: &mut AppBuilder) {
        app.scoped_sub_app(RenderApp, |render_app| {
            render_app
                .add_systems(
                    Extract,
                    ViewDataBuffer::<V>::extract.when::<Exists<ViewDataBuffer<V>>>(),
                )
                .add_systems(Process, ViewDataBuffer::<V>::process);
        })
        .register::<V>()
        .add_render_resource::<ViewDataBuffer<V>>();
    }
}

struct MeshDataPlugin<S: ShaderData>(std::marker::PhantomData<S>);
impl<S: ShaderData> MeshDataPlugin<S> {
    pub fn new() -> Self {
        Self(Default::default())
    }
}

impl<S: ShaderData> Plugin for MeshDataPlugin<S> {
    fn setup(&mut self, app: &mut AppBuilder) {
        app.scoped_sub_app(RenderApp, |render_app| {
            render_app
                .add_systems(PreRender, update_mesh_data_buffers::<S>)
                .add_systems(PostRender, clear_mesh_data_buffers::<S>);
        })
        .add_render_resource::<MeshDataBuffer<S>>()
        .add_render_resource::<BatchedMeshDataBuffer<S>>();
    }
}

pub struct DrawPlugin<D: Draw>(std::marker::PhantomData<D>);
impl<D: Draw> DrawPlugin<D> {
    fn new() -> Self {
        Self(Default::default())
    }
}

pub struct SortViewPhasePlugin<V: View, R: RenderPhase>(std::marker::PhantomData<(V, R)>);
impl<V: View, R: RenderPhase> SortViewPhasePlugin<V, R> {
    fn new() -> Self {
        Self(Default::default())
    }
}
impl<V: View, R: RenderPhase> Plugin for SortViewPhasePlugin<V, R> {
    fn setup(&mut self, app: &mut AppBuilder) {
        app.add_systems(PreRender, ViewDrawCalls::<V, R>::sort);
    }
}

impl<D: Draw> Plugin for DrawPlugin<D> {
    fn setup(&mut self, app: &mut AppBuilder) {
        app.add_plugins((
            RenderPlugin,
            MaterialPlugin::<D::Material>::new(),
            ViewPlugin::<D::View>::new(),
            MeshDataPlugin::<D::Mesh>::new(),
        ))
        .scoped_sub_app(RenderApp, |sub_app| {
            sub_app
                .add_resource(DrawTree::<D>::new())
                .add_resource(VisibleDraws::<D>::new())
                .add_resource(ViewDrawCalls::<D::View, <D::Material as Material>::Phase>::new())
                .add_systems(Extract, DrawTree::<D>::extract)
                .add_systems(PreQueue, VisibleDraws::<D>::queue)
                .add_systems(
                    Queue,
                    ViewDrawCalls::<D::View, <D::Material as Material>::Phase>::queue::<D>
                        .when::<Exists<DrawPipeline<D>>>(),
                )
                .add_systems(
                    PostRender,
                    ViewDrawCalls::<D::View, <D::Material as Material>::Phase>::clear,
                );
        })
        .register::<D>()
        .register::<GlobalTransform>()
        .load_asset::<Shader>(D::shader().into())
        .load_asset::<Shader>(D::Material::shader().into())
        .add_render_resource::<DrawPipeline<D>>();

        if <D::Material as Material>::Phase::SORT {
            app.add_plugins(SortViewPhasePlugin::<
                D::View,
                <D::Material as Material>::Phase,
            >::new());
        }

        if D::CULL {
            app.register::<DisableCulling>();
        }
    }
}
