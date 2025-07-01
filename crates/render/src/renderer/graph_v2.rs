use crate::{PipelineCache, RenderDevice, RenderSurface};
use ecs::{Entity, IndexDag, IndexMap, SystemMeta, World, core::ImmutableIndexDag};
use math::Size;
use std::{any::Any, collections::HashMap};

pub trait GraphResource: Any + Sized + Send + Sync + 'static {
    type Desc: Any + Send + Sync + 'static;

    const NAME: &'static str;

    fn create(device: &RenderDevice, surface: &RenderSurface, desc: &Self::Desc) -> Self;

    fn default_desc() -> Self::Desc;
}

pub trait GraphPass {
    const NAME: &'static str;

    fn setup(
        self,
        builder: &mut PassBuilder,
    ) -> impl Fn(&mut RenderContext) + Send + Sync + 'static;
}

pub trait SubGraph: Send + Sync + 'static {
    const NAME: &'static str;

    fn run() {}
}

pub type NodeId = u32;
pub type DynObject = Box<dyn Any + Send + Sync + 'static>;
pub type PassExecutor = Box<dyn Fn(&mut RenderContext) + Send + Sync + 'static>;
pub type CreateResource = fn(&RenderDevice, &RenderSurface, &DynObject) -> DynObject;
pub type DefaultDesc = fn() -> DynObject;

pub struct PassEdge {
    pub from: NodeId,
    pub to: NodeId,
}

impl From<(NodeId, NodeId)> for PassEdge {
    fn from((from, to): (NodeId, NodeId)) -> Self {
        Self { from, to }
    }
}

pub struct PassNode {
    pub id: NodeId,
    pub name: &'static str,
    pub creates: Vec<NodeId>,
    pub reads: Vec<NodeId>,
    pub writes: Vec<NodeId>,
    pub has_side_effect: bool,
    executor: PassExecutor,
}

impl PassNode {
    fn get_edge(&self, other: &Self) -> Option<PassEdge> {
        if self.id == other.id {
            return None;
        }

        if self
            .creates
            .iter()
            .any(|id| other.reads.contains(id) || other.writes.contains(id))
        {
            return Some(PassEdge::from((self.id, other.id)));
        }

        if other
            .creates
            .iter()
            .any(|id| self.reads.contains(id) || self.writes.contains(id))
        {
            return Some(PassEdge::from((other.id, self.id)));
        }

        if self.reads.iter().any(|id| other.writes.contains(id)) {
            return Some(PassEdge::from((self.id, other.id)));
        }

        if self.writes.iter().any(|id| other.reads.contains(id)) {
            return Some(PassEdge::from((self.id, other.id)));
        }

        if self.writes.iter().any(|id| other.writes.contains(id)) {
            return Some(PassEdge::from((self.id, other.id)));
        }

        None
    }

    fn execute(&self, ctx: &mut RenderContext) {
        (self.executor)(ctx);
    }
}

pub struct ResourceEntry {
    pub id: NodeId,
    pub name: &'static str,
    desc: DynObject,
    create: CreateResource,
}

impl ResourceEntry {
    pub fn new<R: GraphResource>(id: NodeId, desc: R::Desc) -> Self {
        Self {
            id,
            name: R::NAME,
            desc: Box::new(desc),
            create: |device, surface, desc| {
                let desc = desc.downcast_ref::<R::Desc>().unwrap();
                let resource = R::create(device, surface, desc);
                Box::new(resource)
            },
        }
    }
}

pub struct ResourceNode {
    pub id: NodeId,
    pub name: &'static str,
    desc: DynObject,
    object: DynObject,
    create: CreateResource,
}

impl ResourceNode {
    fn new(device: &RenderDevice, surface: &RenderSurface, entry: ResourceEntry) -> Self {
        let object = (entry.create)(device, surface, &entry.desc);

        Self {
            id: entry.id,
            name: entry.name,
            desc: entry.desc,
            object,
            create: entry.create,
        }
    }

    fn create(&mut self, device: &RenderDevice, surface: &RenderSurface) {
        self.object = (self.create)(device, surface, &self.desc)
    }
}

pub struct GraphResourceId<R: GraphResource>(usize, std::marker::PhantomData<R>);
impl<R: GraphResource> Copy for GraphResourceId<R> {}
impl<R: GraphResource> Clone for GraphResourceId<R> {
    fn clone(&self) -> Self {
        Self(self.0.clone(), self.1.clone())
    }
}
impl<R: GraphResource> Eq for GraphResourceId<R> {}
impl<R: GraphResource> PartialEq for GraphResourceId<R> {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}
impl<R: GraphResource> Ord for GraphResourceId<R> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.1.cmp(&other.1)
    }
}
impl<R: GraphResource> PartialOrd for GraphResourceId<R> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        match self.0.partial_cmp(&other.0) {
            Some(core::cmp::Ordering::Equal) => {}
            ord => return ord,
        }
        self.1.partial_cmp(&other.1)
    }
}
impl<R: GraphResource> std::fmt::Debug for GraphResourceId<R> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple(&format!("GraphResourceId<{}>", R::NAME))
            .field(&self.0)
            .finish()
    }
}

impl<R: GraphResource> GraphResourceId<R> {
    fn new(id: usize) -> Self {
        Self(id, Default::default())
    }
}

pub struct SubGraphBuilder {
    passes: IndexMap<&'static str, PassNode>,
    dependencies: IndexDag<NodeId>,
}

pub struct PassBuilder<'a> {
    id: NodeId,
    name: &'static str,
    creates: Vec<NodeId>,
    reads: Vec<NodeId>,
    writes: Vec<NodeId>,
    has_side_effect: bool,
    graph: &'a mut RenderGraphBuilder,
}

impl<'a> PassBuilder<'a> {
    pub fn new(id: NodeId, name: &'static str, graph: &'a mut RenderGraphBuilder) -> Self {
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

    pub fn creates<R: GraphResource>(&mut self, desc: R::Desc) -> GraphResourceId<R> {
        let id = self.graph.add_resource(desc);
        self.creates.push(id.0 as u32);
        id
    }

    pub fn reads<R: GraphResource>(&mut self) -> GraphResourceId<R> {
        let id = match self.graph.resources.get_index_of(R::NAME) {
            Some(index) => GraphResourceId::new(index),
            None => self.creates::<R>(R::default_desc()),
        };

        self.reads.push(id.0 as u32);

        id
    }

    pub fn writes<R: GraphResource>(&mut self) -> GraphResourceId<R> {
        let id = match self.graph.resources.get_index_of(R::NAME) {
            Some(index) => GraphResourceId::new(index),
            None => self.creates::<R>(R::default_desc()),
        };

        self.writes.push(id.0 as u32);

        id
    }

    fn has_side_effect(&mut self) {
        self.has_side_effect = true;
    }

    fn build<P: GraphPass>(mut self, pass: P) -> PassNode {
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

#[derive(Default)]
pub struct RenderGraphBuilder {
    resources: IndexMap<&'static str, ResourceEntry>,
    passes: IndexMap<&'static str, PassNode>,
    sub_graphs: HashMap<&'static str, SubGraphBuilder>,
    dependencies: IndexDag<NodeId>,
}

impl RenderGraphBuilder {
    pub fn add_pass<P: GraphPass>(&mut self, pass: P) -> NodeId {
        todo!()
    }

    pub fn add_sub_graph<S: SubGraph>(&mut self) -> NodeId {
        todo!()
    }

    pub fn add_sub_graph_pass<S: SubGraph, P: GraphPass>(&mut self, pass: P) -> NodeId {
        todo!()
    }

    pub fn add_resource<R: GraphResource>(&mut self, desc: R::Desc) -> GraphResourceId<R> {
        let index = match self.resources.get_index_of(R::NAME) {
            Some(index) => index as usize,
            None => self.resources.len(),
        };

        let resource = ResourceEntry::new::<R>(index as u32, desc);
        self.resources.insert(R::NAME, resource);
        GraphResourceId::new(index)
    }

    pub(crate) fn build(self, device: &RenderDevice, surface: &RenderSurface) -> RenderGraph {
        let Self {
            resources,
            passes,
            dependencies,
            mut sub_graphs,
        } = self;

        let resources = resources
            .into_values()
            .map(|e| ResourceNode::new(device, surface, e));

        let passes = Self::compile_passes(passes, dependencies);

        let sub_graphs = sub_graphs.drain().map(|(name, graph)| {
            let SubGraphBuilder {
                passes,
                dependencies,
            } = graph;

            let passes = Self::compile_passes(passes, dependencies);

            (name, passes)
        });

        RenderGraph {
            resources: resources.collect(),
            passes,
            sub_graphs: sub_graphs.collect(),
            size: surface.size(),
        }
    }

    // TODO: Cull passes that do not effect final output
    fn compile_passes(
        mut passes: IndexMap<&'static str, PassNode>,
        mut dependencies: IndexDag<NodeId>,
    ) -> ImmutableIndexDag<PassNode> {
        let pass_names = passes.keys().copied().collect::<Vec<_>>();

        for (index, pass) in passes.values().enumerate() {
            for next_pass in passes.values().skip(index) {
                if let Some(edge) = pass.get_edge(next_pass) {
                    let from = match dependencies.get_index_of(&edge.from) {
                        Some(from) => from,
                        None => dependencies.add_node(edge.from),
                    };

                    let to = match dependencies.get_index_of(&edge.to) {
                        Some(to) => to,
                        None => dependencies.add_node(edge.to),
                    };

                    dependencies.add_dependency(from, to);
                }
            }
        }

        let _ = dependencies.build();

        dependencies
            .map(|id| passes.shift_remove(pass_names[id as usize]).unwrap())
            .into_immutable()
    }
}

pub struct RenderContext<'a> {
    view: Option<Entity>,
    graph: &'a RenderGraph,
    world: &'a World,
    device: &'a RenderDevice,
    surface: &'a RenderSurface,
    pipelines: &'a PipelineCache,
    meta: &'a SystemMeta,
    buffers: Vec<wgpu::CommandBuffer>,
}

impl<'a> RenderContext<'a> {
    pub fn new(
        graph: &'a RenderGraph,
        world: &'a World,
        device: &'a RenderDevice,
        surface: &'a RenderSurface,
        pipelines: &'a PipelineCache,
        meta: &'a SystemMeta,
    ) -> Self {
        Self {
            view: None,
            graph,
            world,
            device,
            surface,
            meta,
            pipelines,
            buffers: Vec::new(),
        }
    }

    fn run_sub_graph(&mut self, name: &'static str) {
        if let Some(graph) = self.graph.sub_graphs.get(name) {
            for pass in graph.iter() {
                pass.execute(self);
            }
        }
    }
}

pub struct RenderGraph {
    resources: Vec<ResourceNode>,
    passes: ImmutableIndexDag<PassNode>,
    sub_graphs: HashMap<&'static str, ImmutableIndexDag<PassNode>>,
    size: Size<u32>,
}

impl RenderGraph {
    pub fn run(
        &mut self,
        world: &World,
        device: &RenderDevice,
        surface: &RenderSurface,
        piplines: &PipelineCache,
        meta: &SystemMeta,
    ) {
        if self.size != surface.size() {
            for resource in &mut self.resources {
                resource.create(device, surface);
            }
        }

        let mut ctx = RenderContext::new(self, world, device, surface, piplines, meta);

        for pass in self.passes.iter() {
            pass.execute(&mut ctx);
        }

        device.queue.submit(ctx.buffers);
    }
}
