use crate::{
    app::{
        PostRender, PreRender, Present, Process, ProcessPipelines, Queue, QueueDraws, QueueViews,
        Render, RenderApp,
    },
    renderer::{
        Draw, DrawItem, DrawMode, DrawPipeline, ExtractedDraws, ExtractedViews, MeshDataBuffer,
        MeshPipeline, RenderGraph, RenderGraphPass, SubGraph, View, ViewBuffer, ViewDrawCalls,
    },
    resources::{
        AssetExtractors, ExtractInfo, Fallbacks, Material, MaterialLayout, Mesh, PipelineCache,
        RenderAssetExtractor, RenderAssets, RenderItem, RenderResource, RenderTexture,
        ResourceExtractors, ShaderSource, Texture,
    },
    surface::{RenderSurface, RenderSurfaceTexture},
};
use asset::plugin::AssetAppExt;
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
            .add_systems(Extract, RenderSurface::resize_surface)
            .add_systems(Queue, RenderSurface::queue_surface)
            .add_systems(Render, RenderGraph::run_graph)
            .add_systems(Present, RenderSurface::present_surface)
            .add_resource(RenderGraph::new())
            .add_resource(RenderSurfaceTexture::new())
            .add_resource(AssetExtractors::default())
            .add_resource(ResourceExtractors::default())
            .add_resource(PipelineCache::default());

        app.extract_render_resource::<Fallbacks>()
            .extract_render_asset::<ShaderSource>()
            .extract_render_asset::<Mesh>()
            .extract_render_asset::<Texture>()
            .extract_render_asset::<RenderTexture>();
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
    fn add_pass<P: RenderGraphPass>(&mut self, pass: P) -> &mut Self;
    fn add_sub_graph<S: SubGraph>(&mut self) -> &mut Self;
    fn add_sub_graph_pass<S: SubGraph, P: RenderGraphPass>(&mut self, pass: P) -> &mut Self;
    fn extract_render_asset<R: RenderAssetExtractor>(&mut self) -> &mut Self;
    fn extract_render_resource<R: RenderResource>(&mut self) -> &mut Self;
}

impl RenderAppExt for AppBuilder {
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
                .get_sub_graph_mut::<S>()
                .add_pass(pass);
        })
    }

    fn extract_render_asset<R: RenderAssetExtractor>(&mut self) -> &mut Self {
        let app = self.sub_app_mut(RenderApp).unwrap();
        app.resource_mut::<AssetExtractors>().add::<R>();

        if !app.resources().contains::<RenderAssets<R::RenderAsset>>() {
            app.add_resource(RenderAssets::<R::RenderAsset>::new());
        }

        if !app.resources().contains::<ExtractInfo<R>>() {
            app.add_resource(ExtractInfo::<R>::new());
        }

        self.register_asset::<R>()
    }

    fn extract_render_resource<R: RenderResource>(&mut self) -> &mut Self {
        self.get_or_insert_resource(|| ResourceExtractors::default())
            .add::<R>();
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
            .extract_render_asset::<M>();
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
                .add_resource(ExtractedViews::<V>::new())
                .add_systems(Extract, ExtractedViews::<V>::extract)
                .add_systems(QueueViews, ExtractedViews::<V>::queue)
                .add_systems(Process, ViewBuffer::<V>::process)
                .add_systems(PostRender, ViewBuffer::<V>::clear_buffer);
        })
        .extract_render_resource::<ViewBuffer<V>>();
    }
}

struct MeshPipelinePlugin<P: MeshPipeline>(std::marker::PhantomData<P>);
impl<P: MeshPipeline> MeshPipelinePlugin<P> {
    pub fn new() -> Self {
        Self(Default::default())
    }
}

impl<P: MeshPipeline> Plugin for MeshPipelinePlugin<P> {
    fn setup(&mut self, app: &mut AppBuilder) {
        app.scoped_sub_app(RenderApp, |render_app| {
            render_app
                .add_systems(Process, MeshDataBuffer::<P::Mesh>::process)
                .add_systems(PostRender, MeshDataBuffer::<P::Mesh>::clear_buffer);
        })
        .extract_render_resource::<MeshDataBuffer<P::Mesh>>();
    }
}

pub struct DrawPlugin<D: Draw>(std::marker::PhantomData<D>);
impl<D: Draw> DrawPlugin<D> {
    pub fn new() -> Self {
        Self(Default::default())
    }
}

impl<D: Draw> Plugin for DrawPlugin<D> {
    fn setup(&mut self, app: &mut AppBuilder) {
        app.add_plugins((
            RenderPlugin,
            MaterialPlugin::<D::Material>::new(),
            ViewPlugin::<D::View>::new(),
            MeshPipelinePlugin::<D::Pipeline>::new(),
        ))
        .scoped_sub_app(RenderApp, |sub_app| {
            sub_app
                .add_resource(ExtractedDraws::<D>::new())
                .add_resource(ViewDrawCalls::<D::View, DrawMode<D>>::new())
                .add_systems(Extract, ExtractedDraws::<D>::extract)
                .add_systems(
                    QueueDraws,
                    ViewDrawCalls::<D::View, DrawMode<D>>::queue::<D>,
                )
                .add_systems(
                    PostRender,
                    ViewDrawCalls::<D::View, DrawMode<D>>::clear_draws,
                );

            if DrawItem::<D>::SORT {
                sub_app.add_systems(Process, ViewDrawCalls::<D::View, DrawMode<D>>::sort);
            }
        })
        .extract_render_resource::<DrawPipeline<D>>();
    }
}
