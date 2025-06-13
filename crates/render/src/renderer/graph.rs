use crate::{
    Label, RenderState,
    device::RenderDevice,
    renderer::camera::EntityCamera,
    resources::{ComputePipeline, PipelineCache, PipelineId, RenderPipeline},
    surface::{RenderSurface, RenderSurfaceTexture},
};
use ecs::{IndexMap, Resource, world::World};
use std::{
    any::{Any, TypeId},
    collections::HashMap,
    sync::Arc,
};
use wgpu::{RenderPassColorAttachment, RenderPassDepthStencilAttachment};

pub type Name = &'static str;
pub type NodeId = u32;
pub type GraphResourceId = u32;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ResourceType {
    Imported,
    Transient,
}

pub trait GraphResource: Any + Sized + Send + Sync + 'static {
    type Desc: Any + Send + Sync + 'static;

    const NAME: Name;

    fn create(
        device: &RenderDevice,
        surface: &RenderSurface,
        name: Name,
        desc: &Self::Desc,
    ) -> Self;
}

#[derive(Debug, Clone)]
pub struct ResourceNode {
    pub id: NodeId,
    pub resource: GraphResourceId,
    pub version: u32,
}

impl ResourceNode {
    pub fn new(id: NodeId, resource: GraphResourceId) -> Self {
        Self {
            id,
            resource,
            version: 0,
        }
    }
}

pub type ResourceObj = Box<dyn Any + Send + Sync + 'static>;

pub type CreateResource =
    fn(&RenderDevice, &RenderSurface, Name, &(dyn Any + Send + Sync)) -> ResourceObj;

pub struct ResourceEntry {
    pub id: GraphResourceId,
    pub name: Name,
    pub version: u32,
    pub ty: ResourceType,
    desc: Arc<dyn Any + Send + Sync>,
    object: Option<ResourceObj>,
    creator: Option<NodeId>,
    last_pass: Option<NodeId>,
    create: CreateResource,
}

impl ResourceEntry {
    pub fn new<R: GraphResource>(
        id: GraphResourceId,
        name: Name,
        ty: ResourceType,
        desc: R::Desc,
    ) -> Self {
        Self {
            id,
            name,
            version: 0,
            ty,
            desc: Arc::new(desc),
            object: None,
            creator: None,
            last_pass: None,
            create: |device, surface, name, desc| {
                let desc = desc.downcast_ref::<R::Desc>().unwrap();
                let resource = R::create(device, surface, name, desc);
                Box::new(resource)
            },
        }
    }

    pub fn import<R: GraphResource>(id: GraphResourceId, object: Option<R>) -> Self {
        Self {
            id,
            name: R::NAME,
            version: 0,
            ty: ResourceType::Imported,
            desc: Arc::new(()),
            object: object.map(|o| Box::new(o) as ResourceObj),
            creator: None,
            last_pass: None,
            create: |_, _, _, _| unreachable!("imported resources cannot be created"),
        }
    }

    pub fn create(&mut self, device: &RenderDevice, surface: &RenderSurface) {
        let object = (self.create)(device, surface, self.name, self.desc.as_ref());
        self.object = Some(object)
    }

    pub fn inc_version(&mut self) -> u32 {
        self.version += 1;
        self.version
    }

    pub fn destroy(&mut self) {
        self.object = None;
        self.last_pass = None;
        self.creator = None;
    }
}

pub type PassExecutor = Box<dyn Fn(&mut RenderContext) + Send + Sync + 'static>;

pub struct PassNode {
    pub id: NodeId,
    pub name: Name,
    pub creates: Vec<NodeId>,
    pub reads: Vec<NodeId>,
    pub writes: Vec<NodeId>,
    pub has_side_effect: bool,
    executor: PassExecutor,
}

impl PassNode {
    pub fn execute(&self, ctx: &mut RenderContext) {
        (self.executor)(ctx);
    }
}

pub struct PassBuilder<'a> {
    id: NodeId,
    name: Name,
    creates: Vec<GraphResourceId>,
    reads: Vec<GraphResourceId>,
    writes: Vec<GraphResourceId>,
    has_side_effect: bool,
    graph: &'a mut RenderGraph,
}

impl<'a> PassBuilder<'a> {
    pub fn new(id: NodeId, name: Name, graph: &'a mut RenderGraph) -> Self {
        Self {
            id,
            name,
            creates: vec![],
            reads: vec![],
            writes: vec![],
            has_side_effect: false,
            graph,
        }
    }

    pub fn create<R: GraphResource>(&mut self, desc: R::Desc) -> GraphResourceId {
        let id = self.graph.resources.len() as u32;
        let entry = ResourceEntry::new::<R>(
            self.graph.entries.len() as u32,
            R::NAME,
            ResourceType::Transient,
            desc,
        );
        let node = ResourceNode::new(id, self.graph.entries.len() as u32);

        self.graph.resources.push(node);
        self.graph.entries.insert(TypeId::of::<R>(), entry);
        self.creates.push(id);

        id
    }

    pub fn read<G: GraphResource>(&mut self) -> NodeId {
        let id = self
            .graph
            .get_resource_entry::<G>()
            .expect("resource not found")
            .id;

        self.reads.push(id);

        id
    }

    pub fn write<G: GraphResource>(&mut self) -> NodeId {
        let entry = match self.graph.get_resource_entry::<G>() {
            Some(entry) => entry,
            None => {
                self.graph.import::<G>(None);
                self.graph
                    .get_resource_entry::<G>()
                    .expect(&format!("resource not found after import: {}", G::NAME))
            }
        };

        let id = entry.id;

        if entry.ty == ResourceType::Imported {
            self.has_side_effect = true;
        }

        if self.creates.contains(&id) {
            self.writes.push(id);
            id
        } else {
            self.reads.push(id);

            let mut node = self.graph.resources[id as usize].clone();
            node.version = self.entry_mut(id).inc_version();

            let id = self.graph.resources.len() as u32;
            node.id = id;

            self.writes.push(id);
            self.graph.resources.push(node);

            id
        }
    }

    pub fn force(&mut self) {
        self.has_side_effect = true;
    }

    fn entry_mut(&mut self, id: GraphResourceId) -> &mut ResourceEntry {
        let node = &self.graph.resources[id as usize];
        &mut self.graph.entries[node.resource as usize]
    }

    fn build<P: RenderGraphPass>(mut self, pass: P) -> PassNode {
        let executor = P::setup(pass, &mut self);
        PassNode {
            id: self.id,
            name: self.name,
            creates: self.creates,
            reads: self.reads,
            writes: self.writes,
            has_side_effect: self.has_side_effect,
            executor: Box::new(executor),
        }
    }
}

pub trait RenderGraphPass {
    const NAME: Name;

    fn setup(
        self,
        builder: &mut PassBuilder,
    ) -> impl Fn(&mut RenderContext) + Send + Sync + 'static;
}

pub trait SubGraph {
    const NAME: Name;

    fn run(ctx: &mut RenderContext) {
        ctx.run_sub_graph(Self::NAME);
    }
}

pub struct SubGraphPass(fn(&mut RenderContext));

impl SubGraphPass {
    fn new<S: SubGraph>() -> Self {
        Self(S::run)
    }
}

impl RenderGraphPass for SubGraphPass {
    const NAME: Name = "SubgraphPass";

    fn setup(self, _: &mut PassBuilder) -> impl Fn(&mut RenderContext) + 'static {
        move |ctx| (self.0)(ctx)
    }
}

#[derive(Debug)]
pub struct ResourceInfo {
    id: GraphResourceId,
    ref_count: u32,
    creator: Option<NodeId>,
    last_pass: Option<NodeId>,
}

impl From<&ResourceEntry> for ResourceInfo {
    fn from(entry: &ResourceEntry) -> Self {
        Self {
            id: entry.id,
            ref_count: 0,
            creator: entry.creator,
            last_pass: entry.last_pass,
        }
    }
}

#[derive(Debug)]
pub struct CompiledGraph {
    passes: Vec<usize>,
    resources: Vec<ResourceInfo>,
}

#[derive(Resource)]
pub struct RenderGraph {
    passes: Vec<PassNode>,
    resources: Vec<ResourceNode>,
    entries: IndexMap<TypeId, ResourceEntry>,
    sub_graphs: HashMap<Name, Vec<PassNode>>,
}

impl RenderGraph {
    pub fn new() -> Self {
        Self {
            passes: vec![],
            resources: vec![],
            entries: IndexMap::new(),
            sub_graphs: HashMap::new(),
        }
    }

    pub fn add_pass<P: RenderGraphPass>(&mut self, pass: P) -> NodeId {
        let id = self.passes.len() as u32;
        let node = PassBuilder::new(id, P::NAME, self).build::<P>(pass);
        self.passes.push(node);

        id
    }

    pub fn add_sub_graph<S: SubGraph>(&mut self) {
        if !self.sub_graphs.contains_key(S::NAME) {
            let id = self.add_pass(SubGraphPass::new::<S>());
            self.passes[id as usize].has_side_effect = true;
            self.passes[id as usize].name = S::NAME;

            self.sub_graphs.insert(S::NAME, Vec::new());
        }
    }

    pub fn add_sub_graph_pass<S: SubGraph, P: RenderGraphPass>(&mut self, pass: P) -> NodeId {
        self.add_sub_graph::<S>();

        let id = self.sub_graphs.get(S::NAME).unwrap().len() as u32;
        let node = PassBuilder::new(id, P::NAME, self).build::<P>(pass);
        self.sub_graphs.get_mut(S::NAME).unwrap().push(node);

        id
    }

    pub fn import<R: GraphResource>(&mut self, resource: Option<R>) {
        let id = self.entries.len() as u32;
        match self.entries.entry(TypeId::of::<R>()) {
            ecs::core::map::Entry::Occupied(mut entry) => {
                let entry = entry.get_mut();
                if entry.ty == ResourceType::Transient {
                    panic!("transient resource already exists: {}", R::NAME);
                } else {
                    entry.object = resource.map(|r| Box::new(r) as ResourceObj);
                }
            }
            ecs::core::map::Entry::Vacant(entry) => {
                let node = ResourceNode::new(self.resources.len() as u32, id);
                let resource = ResourceEntry::import::<R>(id, resource);

                self.resources.push(node);
                entry.insert(resource);
            }
        }
    }

    pub fn destroy<R: GraphResource>(&mut self) {
        if let Some(entry) = self.entries.get_mut(&TypeId::of::<R>()) {
            entry.destroy();
        }
    }

    pub fn get_resource<G: GraphResource>(&self, id: NodeId) -> Option<&G> {
        let node = self.resources.get(id as usize)?;

        self.entries[node.resource as usize]
            .object
            .as_ref()?
            .downcast_ref::<G>()
    }

    pub fn get_resource_entry<G: GraphResource>(&self) -> Option<&ResourceEntry> {
        self.entries.get(&TypeId::of::<G>())
    }

    pub fn run(&mut self, world: &World, device: &RenderDevice, surface: &RenderSurface) {
        let mut ctx = RenderContext::new(self, world, device, surface);
        ctx.run();
    }

    pub(crate) fn run_graph(
        graph: &mut RenderGraph,
        surface_texture: &RenderSurfaceTexture,
        device: &RenderDevice,
        surface: &RenderSurface,
        world: &World,
    ) {
        let Some(output) = surface_texture
            .get()
            .map(|t| t.texture.create_view(&Default::default()))
            .map(RenderOutput::new)
        else {
            return;
        };

        graph.import::<RenderOutput>(Some(output));

        graph.run(world, &device, &surface);

        graph.destroy::<RenderOutput>();
    }
}

pub struct RenderContext<'a> {
    camera: Option<EntityCamera>,
    graph: &'a mut RenderGraph,
    world: &'a World,
    device: &'a RenderDevice,
    surface: &'a RenderSurface,
    pipelines: &'a PipelineCache,
    buffers: Vec<wgpu::CommandBuffer>,
}

impl<'a> RenderContext<'a> {
    pub fn new(
        graph: &'a mut RenderGraph,
        world: &'a World,
        device: &'a RenderDevice,
        surface: &'a RenderSurface,
    ) -> Self {
        Self {
            camera: None,
            graph,
            world,
            device,
            surface,
            pipelines: world.resource::<PipelineCache>(),
            buffers: Vec::new(),
        }
    }

    pub fn camera(&self) -> Option<&EntityCamera> {
        self.camera.as_ref()
    }

    pub fn world(&self) -> &'a World {
        self.world
    }

    pub fn device(&self) -> &'a RenderDevice {
        self.device
    }

    pub fn surface(&self) -> &'a RenderSurface {
        self.surface
    }

    pub fn get_render_pipeline(&self, id: &PipelineId) -> Option<&RenderPipeline> {
        self.pipelines.get_render_pipeline(id)
    }

    pub fn get_compute_pipeline(&self, id: &PipelineId) -> Option<&ComputePipeline> {
        self.pipelines.get_compute_pipeline(id)
    }

    pub fn get<R: GraphResource>(&self, id: GraphResourceId) -> &R {
        self.graph.get_resource::<R>(id).expect(&format!(
            "resource not found: {} with id {}",
            R::NAME,
            id
        ))
    }

    pub fn encoder(&self) -> wgpu::CommandEncoder {
        self.device.create_command_encoder(&Default::default())
    }

    pub fn submit(&mut self, buffer: wgpu::CommandBuffer) {
        self.buffers.push(buffer);
    }

    pub fn finish(self) -> Vec<wgpu::CommandBuffer> {
        self.buffers
    }

    pub(crate) fn set_camera(&mut self, camera: EntityCamera) {
        self.camera = Some(camera);
    }

    pub(crate) fn run_sub_graph(&mut self, name: Name) {
        if let Some(passes) = self.graph.sub_graphs.remove(name) {
            self.run_passes(&passes);
            self.graph.sub_graphs.insert(name, passes);
        }
    }

    fn run(&mut self) {
        let passes = std::mem::take(&mut self.graph.passes);
        self.run_passes(&passes);
        self.graph.passes = passes;
    }

    fn run_passes(&mut self, passes: &[PassNode]) {
        let mut compiled = self.compile(passes);

        for pass in &compiled.passes {
            for id in passes[*pass].creates.iter().copied() {
                let resource = self.graph.resources[id as usize].resource;
                let entry = &mut self.graph.entries[resource as usize];
                if entry.ty == ResourceType::Transient {
                    entry.create(&self.device, &self.surface);
                }
            }

            {
                passes[*pass].execute(self);

                self.device.queue.submit(self.buffers.drain(..));
            }

            compiled.resources.iter_mut().for_each(|info| {
                let destroy = info.last_pass == Some(passes[*pass].id)
                    && self.graph.entries[info.id as usize].ty == ResourceType::Transient;
                if destroy {
                    self.graph.entries[info.id as usize].destroy();
                }
            });
        }
    }

    fn compile(&self, nodes: &[PassNode]) -> CompiledGraph {
        let mut passes = nodes.iter().map(|p| p.writes.len()).collect::<Vec<_>>();
        let mut resources = self
            .graph
            .entries
            .values()
            .map(ResourceInfo::from)
            .collect::<Vec<_>>();

        for pass in nodes {
            for id in &pass.reads {
                let resource = self.graph.resources[*id as usize].resource;
                resources[resource as usize].ref_count += 1;
            }

            for id in &pass.writes {
                let resource = self.graph.resources[*id as usize].resource;
                resources[resource as usize].creator = Some(pass.id);
            }
        }

        let mut unreferenced = resources
            .iter()
            .enumerate()
            .filter_map(|(id, info)| (info.ref_count == 0).then_some(id as u32))
            .collect::<Vec<_>>();

        while let Some(id) = unreferenced.pop() {
            let Some(pass) = resources[id as usize].creator else {
                continue;
            };

            if nodes[pass as usize].has_side_effect {
                continue;
            }

            assert!(passes[pass as usize] >= 1);
            passes[pass as usize] -= 1;
            if passes[pass as usize] == 0 {
                for id in &nodes[pass as usize].reads {
                    resources[*id as usize].ref_count -= 1;
                    if resources[*id as usize].ref_count == 0 {
                        unreferenced.push(*id);
                    }
                }
            }
        }

        let queue = passes.iter().enumerate().filter_map(|(pass, ref_count)| {
            if *ref_count == 0 && !nodes[pass].has_side_effect {
                return None;
            }

            for id in &nodes[pass].creates {
                let resource = self.graph.resources[*id as usize].resource;
                resources[resource as usize].creator = Some(nodes[pass].id);
            }

            for id in &nodes[pass].reads {
                let resource = self.graph.resources[*id as usize].resource;
                resources[resource as usize].last_pass = Some(nodes[pass].id);
            }

            for id in &nodes[pass].writes {
                let resource = self.graph.resources[*id as usize].resource;
                resources[resource as usize].last_pass = Some(nodes[pass].id);
            }

            Some(pass)
        });

        CompiledGraph {
            passes: queue.collect(),
            resources,
        }
    }
}

pub struct RenderOutput(wgpu::TextureView);
impl From<wgpu::TextureView> for RenderOutput {
    fn from(value: wgpu::TextureView) -> Self {
        Self(value)
    }
}

impl RenderOutput {
    pub fn new(view: wgpu::TextureView) -> Self {
        Self(view)
    }
}

impl std::ops::Deref for RenderOutput {
    type Target = wgpu::TextureView;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl GraphResource for RenderOutput {
    type Desc = ();

    const NAME: Name = "RenderTarget";

    fn create(device: &RenderDevice, surface: &RenderSurface, name: Name, _: &Self::Desc) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(name),
            size: wgpu::Extent3d {
                width: surface.width(),
                height: surface.height(),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: surface.format(),
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });

        Self(texture.create_view(&wgpu::TextureViewDescriptor::default()))
    }
}

pub struct DepthOutput(wgpu::TextureView);
impl From<wgpu::TextureView> for DepthOutput {
    fn from(value: wgpu::TextureView) -> Self {
        Self(value)
    }
}

impl DepthOutput {
    pub fn new(view: wgpu::TextureView) -> Self {
        Self(view)
    }
}

impl std::ops::Deref for DepthOutput {
    type Target = wgpu::TextureView;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl GraphResource for DepthOutput {
    type Desc = ();

    const NAME: Name = "DepthTarget";

    fn create(device: &RenderDevice, surface: &RenderSurface, name: Name, _: &Self::Desc) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(name),
            size: wgpu::Extent3d {
                width: surface.width(),
                height: surface.height(),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: surface.depth_format(),
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        Self(texture.create_view(&wgpu::TextureViewDescriptor::default()))
    }
}

#[derive(Clone, Debug, Default)]
pub struct RenderPassDesc<'a> {
    /// Debug label of the render pass. This will show up in graphics debuggers for easy identification.
    pub label: Label,
    /// The color attachments of the render pass.
    pub color_attachments: Vec<Option<RenderPassColorAttachment<'a>>>,
    /// The depth and stencil attachment of the render pass, if any.
    pub depth_stencil_attachment: Option<RenderPassDepthStencilAttachment<'a>>,
}

pub trait Renderer: Send + Sync + 'static {
    type Data: Send + Sync + 'static;

    fn setup(builder: &mut PassBuilder) -> Self::Data;

    fn build<'a>(ctx: &'a RenderContext<'a>, data: &'a Self::Data) -> RenderPassDesc<'a>;

    fn render<'a>(ctx: &'a mut RenderContext, state: RenderState<'a>);
}

pub trait ErasedRenderer: downcast_rs::Downcast + Send + Sync + 'static {
    fn setup(&self, builder: &mut PassBuilder) -> Box<dyn Any + Send + Sync>;

    fn render(&self, ctx: &mut RenderContext, data: &Box<dyn Any + Send + Sync>);
}

impl<R: Renderer> ErasedRenderer for R {
    fn setup(&self, builder: &mut PassBuilder) -> Box<dyn Any + Send + Sync> {
        Box::new(Self::setup(builder))
    }

    fn render(&self, ctx: &mut RenderContext, data: &Box<dyn Any + Send + Sync>) {
        let mut encoder = ctx.encoder();
        let desc = Self::build(ctx, data.downcast_ref::<R::Data>().unwrap());
        let desc = wgpu::RenderPassDescriptor {
            label: desc.label.as_deref(),
            color_attachments: &desc.color_attachments,
            depth_stencil_attachment: desc.depth_stencil_attachment,
            timestamp_writes: None,
            occlusion_query_set: None,
        };

        Self::render(ctx, RenderState::new(encoder.begin_render_pass(&desc)));

        ctx.submit(encoder.finish());
    }
}

downcast_rs::impl_downcast!(ErasedRenderer);

pub struct MainRenderPass {
    renderer: Box<dyn ErasedRenderer>,
}

impl RenderGraphPass for MainRenderPass {
    const NAME: super::graph::Name = "MainRenderPass";

    fn setup(self, builder: &mut PassBuilder) -> impl Fn(&mut RenderContext) + 'static {
        let data = self.renderer.setup(builder);

        move |ctx| {
            self.renderer.render(ctx, &data);
        }
    }
}

impl MainRenderPass {
    pub fn new<R: Renderer>(renderer: R) -> Self {
        Self {
            renderer: Box::new(renderer),
        }
    }
}
