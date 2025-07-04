use crate::{
    ComputePipeline, PipelineCache, PipelineId, RenderDevice, RenderPipeline, RenderSurface,
};
use ecs::{Entity, IndexMap, Resource, SystemMeta, World};
use smol::lock::RwLock;
use std::{
    any::Any,
    collections::{HashMap, HashSet},
    sync::Arc,
};

pub type Name = &'static str;
pub type NodeId = usize;
pub type Bobject = Box<dyn Any + Send + Sync + 'static>;
pub type Executor = Box<dyn Fn(&mut RenderContext) + Send + Sync + 'static>;
pub type Creator = fn(&World, &RenderDevice, &RenderSurface, &Bobject) -> Bobject;
type Passes = IndexMap<Name, PassNode>;

pub enum RenderGraphError {
    MissingResource { name: Name, id: NodeId },
    MissingView,
    Custom(String),
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ResourceType {
    Transient,
    Imported,
}

pub trait GraphResource: Sized + Send + Sync + 'static {
    const NAME: Name;

    type Desc: Default + Send + Sync + 'static;

    fn create(
        world: &World,
        device: &RenderDevice,
        surface: &RenderSurface,
        desc: &Self::Desc,
    ) -> Self;
}

pub struct GraphResourceId<R: GraphResource>(usize, std::marker::PhantomData<R>);
impl<R: GraphResource> From<usize> for GraphResourceId<R> {
    fn from(value: usize) -> Self {
        Self(value, Default::default())
    }
}

impl<R: GraphResource> Copy for GraphResourceId<R> {}
impl<R: GraphResource> Clone for GraphResourceId<R> {
    fn clone(&self) -> Self {
        Self(self.0.clone(), self.1.clone())
    }
}

impl<R: GraphResource> std::fmt::Debug for GraphResourceId<R> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}({})", R::NAME, self.0)
    }
}

impl<R: GraphResource> std::ops::Deref for GraphResourceId<R> {
    type Target = usize;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub trait GraphPass {
    const NAME: Name;

    fn setup(
        self,
        builder: &mut PassBuilder,
    ) -> impl Fn(&mut RenderContext) + Send + Sync + 'static;
}

pub trait SubGraph: Send + Sync + 'static {
    const NAME: Name;

    fn run(ctx: &mut RenderContext) {
        ctx.run_sub_graph(Self::NAME)
    }
}

pub struct SubGraphPass {
    name: Name,
    run: fn(&mut RenderContext),
}

impl SubGraphPass {
    fn new<S: SubGraph>() -> Self {
        Self {
            name: S::NAME,
            run: S::run,
        }
    }
}

impl GraphPass for SubGraphPass {
    const NAME: Name = "SubgraphPass";

    fn setup(self, builder: &mut PassBuilder) -> impl Fn(&mut RenderContext) + 'static {
        builder.name = self.name;

        move |ctx| (self.run)(ctx)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum GraphId {
    Main,
    Sub(Name),
}

pub struct PassNode {
    pub id: NodeId,
    pub name: Name,
    pub reads: Vec<NodeId>,
    pub writes: Vec<NodeId>,
    pub has_side_effect: bool,
    executor: Executor,
}

impl PassNode {
    fn execute(&self, ctx: &mut RenderContext) {
        (self.executor)(ctx)
    }
}

pub struct ResourceNode {
    pub id: NodeId,
    pub ty: ResourceType,
    pub name: Name,
    desc: Bobject,
    object: Option<Bobject>,
    creator: Creator,
}

impl ResourceNode {
    pub fn new<R: GraphResource>(id: NodeId, ty: ResourceType, desc: R::Desc) -> Self {
        Self {
            id,
            ty,
            name: R::NAME,
            desc: Box::new(desc),
            creator: |world, device, surface, desc| {
                let desc = desc.downcast_ref::<R::Desc>().unwrap();
                let resource = R::create(world, device, surface, desc);
                Box::new(resource)
            },
            object: None,
        }
    }

    fn create(&mut self, world: &World, device: &RenderDevice, surface: &RenderSurface) {
        let object = (self.creator)(world, device, surface, &self.desc);
        self.object = Some(object);
    }
}

#[derive(Resource)]
pub struct RenderGraphBuilder {
    passes: Passes,
    resources: IndexMap<Name, ResourceNode>,
    sub_graphs: HashMap<Name, Passes>,
}

impl RenderGraphBuilder {
    pub fn new() -> Self {
        Self {
            passes: Passes::new(),
            resources: IndexMap::new(),
            sub_graphs: HashMap::new(),
        }
    }

    pub fn add_pass<P: GraphPass>(&mut self, pass: P) -> NodeId {
        if let Some(id) = self.passes.get_index_of(P::NAME) {
            return id;
        }

        let id = self.passes.len();
        let pass = PassBuilder::new(id, P::NAME, self).build(pass);
        self.passes.insert(P::NAME, pass);

        id
    }

    pub fn add_sub_graph<S: SubGraph>(&mut self) {
        self.sub_graphs.entry(S::NAME).or_default();
    }

    pub fn add_sub_graph_pass<S: SubGraph, P: GraphPass>(&mut self, pass: P) -> NodeId {
        if let Some(id) = self
            .sub_graphs
            .entry(S::NAME)
            .or_default()
            .get_index_of(P::NAME)
        {
            return id;
        }

        let id = self.sub_graphs.get(S::NAME).unwrap().len();
        let pass = PassBuilder::new(id, P::NAME, self).build(pass);
        self.sub_graphs
            .get_mut(S::NAME)
            .unwrap()
            .insert(P::NAME, pass);

        id
    }

    pub fn import<R: GraphResource>(&mut self, desc: R::Desc) -> GraphResourceId<R> {
        if let Some(id) = self.get_resource_id::<R>() {
            let resource = &mut self.resources[id.0];
            resource.desc = Box::new(desc);
            resource.ty = ResourceType::Imported;
            self.remove_pass(R::NAME);
            id
        } else {
            let id = self.resources.len();
            let resource = ResourceNode::new::<R>(id, ResourceType::Imported, desc);
            self.resources.insert(R::NAME, resource);

            GraphResourceId::from(id)
        }
    }

    pub fn get_resource_id<R: GraphResource>(&self) -> Option<GraphResourceId<R>> {
        self.resources
            .get_index_of(R::NAME)
            .map(GraphResourceId::from)
    }

    pub fn build(self) -> RenderGraph {
        #[derive(Clone, Copy, PartialEq, Eq, Hash)]
        pub struct PassId {
            id: NodeId,
            graph: GraphId,
        }

        struct ResourceInfo {
            ref_count: u32,
            first_user: Option<PassId>,
        }

        impl From<usize> for ResourceInfo {
            fn from(value: usize) -> Self {
                Self {
                    ref_count: value as u32,
                    first_user: None,
                }
            }
        }

        struct PassInfo {
            id: NodeId,
            graph: GraphId,
            ref_count: u32,
        }

        impl PassInfo {
            fn new(id: NodeId, graph: GraphId) -> Self {
                Self {
                    id,
                    graph,
                    ref_count: 0,
                }
            }
        }

        let RenderGraphBuilder {
            passes,
            resources,
            mut sub_graphs,
        } = self;

        let mut graphs = sub_graphs
            .drain()
            .map(|(name, passes)| (GraphId::Sub(name), passes))
            .collect::<HashMap<_, _>>();
        graphs.insert(GraphId::Main, passes);

        let mut resource_refs = (0..resources.len())
            .map(ResourceInfo::from)
            .collect::<Vec<_>>();
        let mut pass_refs = graphs
            .iter()
            .map(|(id, passes)| {
                let refs = (0..passes.len())
                    .map(|i| PassInfo::new(i, *id))
                    .collect::<Vec<_>>();

                (*id, refs)
            })
            .collect::<HashMap<_, _>>();

        for (graph, passes) in &graphs {
            let pass_refs = pass_refs.get_mut(graph).unwrap();

            for pass in passes.values() {
                for id in &pass.reads {
                    resource_refs[*id].ref_count += 1;
                    pass_refs[pass.id].ref_count = pass.writes.len() as u32;
                }

                for id in &pass.writes {
                    if resource_refs[*id].first_user.is_none() {
                        resource_refs[*id].first_user = Some(PassId {
                            id: pass.id,
                            graph: *graph,
                        })
                    }
                }
            }
        }

        let mut unreferenced = resource_refs
            .iter()
            .enumerate()
            .filter_map(|(i, v)| (v.ref_count == 0).then_some(i))
            .collect::<Vec<_>>();

        while let Some(id) = unreferenced.pop() {
            let Some(pass) = resource_refs[id].first_user else {
                continue;
            };

            let passes = graphs.get_mut(&pass.graph).unwrap();

            if passes[pass.id].has_side_effect {
                continue;
            }

            let pass_refs = pass_refs.get_mut(&pass.graph).unwrap();

            assert!(pass_refs[pass.id].ref_count >= 1);
            pass_refs[pass.id].ref_count -= 1;
            if pass_refs[pass.id].ref_count == 0 {
                for id in passes[pass.id].reads.iter().copied() {
                    resource_refs[id].ref_count -= 1;
                    if resource_refs[id].ref_count == 0 {
                        unreferenced.push(id);
                    }
                }
            }
        }

        let mut sub_graphs = graphs
            .drain()
            .map(|(graph, passes)| {
                let pass_refs = pass_refs.get(&graph).unwrap();
                let passes = passes.into_values().filter_map(|p| {
                    (p.has_side_effect || pass_refs[p.id].ref_count > 0).then_some(p)
                });

                (graph, passes.collect::<Vec<_>>())
            })
            .collect::<HashMap<_, _>>();

        let passes = sub_graphs.remove(&GraphId::Main).unwrap();

        RenderGraph {
            resources,
            passes: Arc::new(RwLock::new(passes)),
            sub_graphs: Arc::new(RwLock::new(sub_graphs)),
        }
    }

    fn remove_pass(&mut self, name: Name) {
        if let Some(index) = self.passes.shift_remove(name).map(|p| p.id) {
            for pass in self.passes.values_mut().skip(index) {
                pass.id -= 1;
            }
        }
    }
}

pub struct PassBuilder<'a> {
    id: NodeId,
    name: Name,
    reads: HashSet<NodeId>,
    writes: HashSet<NodeId>,
    has_side_effect: bool,
    builder: &'a mut RenderGraphBuilder,
}

impl<'a> PassBuilder<'a> {
    pub fn new(id: NodeId, name: Name, builder: &'a mut RenderGraphBuilder) -> Self {
        Self {
            id,
            name,
            reads: HashSet::new(),
            writes: HashSet::new(),
            has_side_effect: false,
            builder,
        }
    }

    pub fn id(&self) -> NodeId {
        self.id
    }

    pub fn read<R: GraphResource>(&mut self) -> GraphResourceId<R> {
        let id = self.get_or_insert_resource::<R>();

        self.reads.insert(id.0);

        id
    }

    pub fn write<R: GraphResource>(&mut self) -> GraphResourceId<R> {
        let id = self.get_or_insert_resource::<R>();

        self.writes.insert(id.0);

        id
    }

    pub fn has_side_effect(&mut self) {
        self.has_side_effect = true;
    }

    fn get_or_insert_resource<R: GraphResource>(&mut self) -> GraphResourceId<R> {
        if let Some(id) = self.builder.get_resource_id::<R>() {
            id
        } else {
            let id = self.builder.resources.len();
            let resource = ResourceNode::new::<R>(id, ResourceType::Imported, R::Desc::default());
            self.builder.add_pass(CreateResourcePass::<R>::new());
            self.builder.resources.insert(R::NAME, resource);
            GraphResourceId::from(id)
        }
    }

    fn build<P: GraphPass>(mut self, pass: P) -> PassNode {
        let executor = P::setup(pass, &mut self);
        PassNode {
            id: self.id,
            name: self.name,
            reads: self.reads.drain().collect(),
            writes: self.writes.drain().collect(),
            has_side_effect: self.has_side_effect,
            executor: Box::new(executor),
        }
    }
}

pub struct CreateResourcePass<R: GraphResource>(std::marker::PhantomData<R>);
impl<R: GraphResource> CreateResourcePass<R> {
    fn new() -> Self {
        Self(Default::default())
    }
}

impl<R: GraphResource> GraphPass for CreateResourcePass<R> {
    const NAME: Name = R::NAME;

    fn setup(
        self,
        builder: &mut PassBuilder,
    ) -> impl Fn(&mut RenderContext) + Send + Sync + 'static {
        // Resource is inserted after the pass is created,
        // so we can safely use the builder to get the resource ID.
        let id = builder.builder.resources.len();
        builder.writes.insert(id);

        move |ctx| ctx.graph.resources[id].create(ctx.world, ctx.device, ctx.surface)
    }
}

pub struct RenderContext<'a> {
    view: Option<Entity>,
    graph: &'a mut RenderGraph,
    world: &'a World,
    device: &'a RenderDevice,
    surface: &'a RenderSurface,
    pipelines: &'a PipelineCache,
    meta: &'a SystemMeta,
    buffers: Vec<wgpu::CommandBuffer>,
}

impl<'a> RenderContext<'a> {
    pub fn new(
        graph: &'a mut RenderGraph,
        world: &'a World,
        device: &'a RenderDevice,
        surface: &'a RenderSurface,
        meta: &'a SystemMeta,
    ) -> Self {
        Self {
            view: None,
            graph,
            world,
            device,
            surface,
            meta,
            pipelines: world.resource::<PipelineCache>(),
            buffers: Vec::new(),
        }
    }

    pub fn view(&self) -> Option<Entity> {
        self.view
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

    pub fn meta(&self) -> &'a SystemMeta {
        self.meta
    }

    pub fn get<R: GraphResource>(&self, id: GraphResourceId<R>) -> Option<&R> {
        self.graph.get::<R>(id)
    }

    pub fn get_render_pipeline(&self, id: &PipelineId) -> Option<&RenderPipeline> {
        self.pipelines.get_render_pipeline(id)
    }

    pub fn get_compute_pipeline(&self, id: &PipelineId) -> Option<&ComputePipeline> {
        self.pipelines.get_compute_pipeline(id)
    }

    pub fn encoder(&self) -> wgpu::CommandEncoder {
        self.device.create_command_encoder(&Default::default())
    }

    pub fn submit(&mut self, buffer: wgpu::CommandBuffer) {
        self.buffers.push(buffer);
    }

    pub(crate) fn set_view(&mut self, view: Entity) {
        self.view = Some(view);
    }

    pub(crate) fn run_sub_graph(&mut self, name: Name) {
        let sub_graphs = self.graph.sub_graphs.read_arc_blocking();
        if let Some(passes) = sub_graphs.get(&GraphId::Sub(name)) {
            for pass in passes {
                pass.execute(self);
            }
        }
    }

    fn run(mut self) -> Result<Vec<wgpu::CommandBuffer>, RenderGraphError> {
        let passes = self.graph.passes.read_arc_blocking();
        for pass in passes.iter() {
            pass.execute(&mut self);
        }

        Ok(self.buffers)
    }
}

#[derive(Resource)]
pub struct RenderGraph {
    resources: IndexMap<Name, ResourceNode>,
    passes: Arc<RwLock<Vec<PassNode>>>,
    sub_graphs: Arc<RwLock<HashMap<GraphId, Vec<PassNode>>>>,
}

impl RenderGraph {
    pub fn get<R: GraphResource>(&self, id: GraphResourceId<R>) -> Option<&R> {
        self.resources[*id].object.as_ref()?.downcast_ref::<R>()
    }

    pub fn import<R: GraphResource>(&mut self, resource: R) {
        if let Some(node) = self
            .resources
            .get_mut(R::NAME)
            .filter(|r| r.ty == ResourceType::Imported)
        {
            node.object = Some(Box::new(resource));
        }
    }

    pub fn run(
        &mut self,
        world: &World,
        device: &RenderDevice,
        surface: &RenderSurface,
        meta: &SystemMeta,
    ) {
        if let Ok(buffers) = RenderContext::new(self, world, device, surface, meta).run() {
            device.queue.submit(buffers);
        }
    }

    pub(crate) fn run_graph(
        graph: &mut RenderGraph,
        world: &World,
        device: &RenderDevice,
        surface: &RenderSurface,
        meta: &SystemMeta,
    ) {
        graph.run(world, device, surface, meta);
    }
}
