use crate::{
    CameraRenderTargets, CameraSubGraph, DisableCulling, Draw, DrawPipeline, DrawTree, DrawView,
    ExtractError, GpuTexture, LightingData, MaterialBinding, ObjImporter, ProcessAssets,
    QueueDraws, QueueViews, RenderGraphBuilder, RenderMesh, RenderTarget, GpuShader, SubMesh,
    Texture2dImporter, ViewDrawCalls, VisibleDraws,
    app::{PostRender, PreRender, Present, Process, Queue, Render, RenderApp},
    draw::{
        Renderer, RendererPass,
        material::{Material, MaterialLayout, RenderPhase},
        model::{
            BatchedModelDataBuffer, ModelData, ModelDataBuffer, clear_model_data_buffers,
            update_model_data_buffers,
        },
        view::{RenderViews, View},
    },
    renderer::{Camera, GraphPass, RenderGraph, SubGraph},
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
use transform::GlobalTransform;
use window::plugin::WindowPlugin;

pub struct RenderPlugin;

impl Plugin for RenderPlugin {
    fn setup(&mut self, app: &mut ecs::AppBuilder) {
        app.add_plugins((WindowPlugin, AssetPlugin))
            .add_sub_app(RenderApp)
            .add_sub_phase(Run, Process)
            .add_sub_phase(Process, ProcessAssets)
            .add_sub_phase(Run, Queue)
            .add_sub_phase(Queue, QueueViews)
            .add_sub_phase(Queue, QueueDraws)
            .add_sub_phase(Run, PreRender)
            .add_sub_phase(Run, Render)
            .add_sub_phase(Run, Present)
            .add_sub_phase(Run, PostRender)
            .add_systems(Init, RenderSurface::create_surface)
            .add_systems(Init, RenderGraph::create_graph)
            .add_systems(Extract, RenderSurface::resize_surface)
            .add_systems(Queue, RenderSurface::queue_surface)
            .add_systems(Render, RenderGraph::run_graph)
            .add_systems(Present, RenderSurface::present_surface)
            .add_resource(RenderGraphBuilder::new())
            .add_resource(RenderSurfaceTexture::new())
            .add_resource(AssetExtractors::default())
            .add_resource(ResourceExtractors::default())
            .add_resource(PipelineCache::default())
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
            .set_default_processor::<Mesh>();
    }

    fn finish(&mut self, app: &mut AppBuilder) {
        if !app.resources().contains::<ResourceExtractors>() {
            app.add_resource(ResourceExtractors::default());
        }

        let app = app.sub_app_mut(RenderApp).unwrap();
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
    fn register_draw<D: Draw>(&mut self) -> &mut Self;
    fn add_pass<P: GraphPass>(&mut self, pass: P) -> &mut Self;
    fn add_sub_graph<S: SubGraph>(&mut self) -> &mut Self;
    fn add_sub_graph_pass<S: SubGraph, P: GraphPass>(&mut self, pass: P) -> &mut Self;
    fn add_renderer<R: Renderer>(&mut self) -> &mut Self;
    fn add_render_asset<R: RenderAsset>(&mut self) -> &mut Self;
    fn add_render_resource<R: RenderResource>(&mut self) -> &mut Self;
}

impl RenderAppExt for AppBuilder {
    fn register_draw<D: Draw>(&mut self) -> &mut Self {
        self.add_plugins(DrawPlugin::<D>::new())
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
                .add_sub_graph_pass::<CameraSubGraph, RendererPass<R>>(RendererPass::default());
        });

        self.add_plugins(CameraPlugin)
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

pub struct CameraPlugin;
impl Plugin for CameraPlugin {
    fn setup(&mut self, app: &mut AppBuilder) {
        app.add_plugins(SyncComponentPlugin::<Camera, RenderApp>::new())
            .sub_app_mut(RenderApp)
            .unwrap()
            .add_resource(CameraRenderTargets::default())
            .add_systems(PreRender, CameraRenderTargets::queue)
            .add_systems(PostRender, CameraRenderTargets::cleanup);
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
        app.add_plugins(CameraPlugin);
        app.scoped_sub_app(RenderApp, |render_app| {
            render_app
                .add_systems(
                    Extract,
                    RenderViews::<V>::extract.when::<Exists<RenderViews<V>>>(),
                )
                .add_systems(Process, RenderViews::<V>::process);
        })
        .register::<V>()
        .add_render_resource::<RenderViews<V>>();
    }
}

struct ModelDataPlugin<M: ModelData>(std::marker::PhantomData<M>);
impl<M: ModelData> ModelDataPlugin<M> {
    pub fn new() -> Self {
        Self(Default::default())
    }
}

impl<M: ModelData> Plugin for ModelDataPlugin<M> {
    fn setup(&mut self, app: &mut AppBuilder) {
        app.scoped_sub_app(RenderApp, |render_app| {
            render_app
                .add_systems(PreRender, update_model_data_buffers::<M>)
                .add_systems(PostRender, clear_model_data_buffers::<M>);
        })
        .add_render_resource::<ModelDataBuffer<M>>()
        .add_render_resource::<BatchedModelDataBuffer<M>>();
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
        app.add_plugins(ViewPlugin::<V>::new())
            .add_systems(PreRender, ViewDrawCalls::<V, R>::sort);
    }
}

impl<D: Draw> Plugin for DrawPlugin<D> {
    fn setup(&mut self, app: &mut AppBuilder) {
        app.add_plugins((
            RenderPlugin,
            CameraPlugin,
            MaterialPlugin::<D::Material>::new(),
            ViewPlugin::<DrawView<D>>::new(),
            ModelDataPlugin::<D::Model>::new(),
        ))
        .scoped_sub_app(RenderApp, |sub_app| {
            sub_app
                .add_resource(DrawTree::<D>::new())
                .add_resource(VisibleDraws::<D>::new())
                .add_resource(ViewDrawCalls::<DrawView<D>, <D::Material as Material>::Phase>::new())
                .add_systems(Extract, DrawTree::<D>::extract)
                .add_systems(QueueViews, VisibleDraws::<D>::queue)
                .add_systems(
                    QueueDraws,
                    ViewDrawCalls::<DrawView<D>, <D::Material as Material>::Phase>::queue::<
                        <D::Material as Material>::Lighting,
                        D::Material,
                        D,
                    >
                        .when::<Exists<DrawPipeline<D>>>(),
                )
                .add_systems(
                    PostRender,
                    ViewDrawCalls::<DrawView<D>, <D::Material as Material>::Phase>::clear,
                );
        })
        .register::<D>()
        .register::<GlobalTransform>()
        .load_asset::<GpuShader>(D::shader().into())
        .load_asset::<GpuShader>(D::Material::shader().into())
        .add_render_resource::<LightingData<<D::Material as Material>::Lighting>>()
        .add_render_resource::<DrawPipeline<D>>();

        if <D::Material as Material>::Phase::SORT {
            app.add_plugins(SortViewPhasePlugin::<
                DrawView<D>,
                <D::Material as Material>::Phase,
            >::new());
        }

        if D::CULL {
            app.register::<DisableCulling>();
        }
    }
}
