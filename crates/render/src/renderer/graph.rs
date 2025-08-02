use crate::{
    CameraAttachments, ComputePipeline, PipelineCache, PipelineId, RenderDevice, RenderPipeline,
    RenderSurface, RenderSurfaceTexture,
};
use ecs::{
    Commands, Entity, EventReader, IndexDag, IndexMap, Resource, World, core::ImmutableIndexDag,
    system::Main,
};
use smol::lock::RwLock;
use std::{
    any::Any,
    collections::{HashMap, HashSet},
    error::Error,
    sync::Arc,
};
use window::events::WindowResized;

pub type Name = &'static str;
pub type NodeId = usize;
pub type Bobject = Box<dyn Any + Send + Sync + 'static>;
pub type Executor =
    Box<dyn Fn(&mut RenderContext) -> Result<(), RenderGraphError> + Send + Sync + 'static>;
pub type Creator =
    fn(&World, &RenderDevice, &RenderSurface, &Bobject) -> Result<Bobject, RenderGraphError>;
type Passes = IndexMap<Name, PassNode>;

#[derive(Clone, Debug, thiserror::Error)]
pub enum RenderGraphError {
    #[error("Failed to build render graph")]
    Build,
    #[error("Missing Render View")]
    MissingView,
    #[error("Missing Pipeline: {:?}", id)]
    MissingPipeline { id: PipelineId },
    #[error("Missing Resource: {resource} for pass {pass}")]
    MissingResource { pass: Name, resource: NodeId },
    #[error("Missing Render target for camera: {entity}")]
    MissingRenderTarget { entity: Entity },
    #[error("Render graph error: {0}")]
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

    type Error: Error + Send + Sync + 'static;

    fn create(
        world: &World,
        device: &RenderDevice,
        surface: &RenderSurface,
        desc: &Self::Desc,
    ) -> Result<Self, Self::Error>;
}

pub struct GraphResourceId<R: GraphResource>(usize, std::marker::PhantomData<R>);
impl<R: GraphResource> GraphResourceId<R> {
    pub const NONE: Self = Self(usize::MAX, std::marker::PhantomData);
}

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
    ) -> impl Fn(&mut RenderContext) -> Result<(), RenderGraphError> + Send + Sync + 'static;
}

pub trait SubGraph: Send + Sync + 'static {
    const NAME: Name;

    fn run(ctx: &mut RenderContext) -> Result<(), RenderGraphError> {
        ctx.run_sub_graph(Self::NAME)
    }
}

pub struct SubGraphPass {
    name: Name,
    run: fn(&mut RenderContext) -> Result<(), RenderGraphError>,
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

    fn setup(
        self,
        builder: &mut PassBuilder,
    ) -> impl Fn(&mut RenderContext) -> Result<(), RenderGraphError> + 'static {
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
    pub dependencies: Vec<Name>,
    pub has_side_effect: bool,
    executor: Executor,
}

impl PassNode {
    fn execute(&self, ctx: &mut RenderContext) -> Result<(), RenderGraphError> {
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
                let desc = desc
                    .downcast_ref::<R::Desc>()
                    .ok_or(RenderGraphError::Custom("".to_string()))?;
                let resource = R::create(world, device, surface, desc);
                Ok(Box::new(resource))
            },
            object: None,
        }
    }

    fn create(
        &mut self,
        world: &World,
        device: &RenderDevice,
        surface: &RenderSurface,
    ) -> Result<(), RenderGraphError> {
        let object = (self.creator)(world, device, surface, &self.desc)?;
        self.object = Some(object);
        Ok(())
    }

    fn destroy(&mut self) {
        self.object = None;
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
        self.passes.insert(pass.name, pass);

        id
    }

    pub fn add_sub_graph<S: SubGraph>(&mut self) {
        if !self.sub_graphs.contains_key(S::NAME) {
            let id = self.add_pass(SubGraphPass::new::<S>());
            self.passes[id].has_side_effect = true;
            self.sub_graphs.insert(S::NAME, IndexMap::new());
        }
    }

    pub fn add_sub_graph_pass<S: SubGraph, P: GraphPass>(&mut self, pass: P) -> NodeId {
        if let Some(id) = self
            .sub_graphs
            .get(S::NAME)
            .and_then(|g| g.get_index_of(P::NAME))
        {
            return id;
        }

        self.add_sub_graph::<S>();

        let id = self.sub_graphs.get(S::NAME).unwrap().len();
        let pass = PassBuilder::new(id, P::NAME, self).build(pass);
        self.sub_graphs
            .get_mut(S::NAME)
            .unwrap()
            .insert(P::NAME, pass);

        id
    }

    pub fn add_nested_sub_graph<S: SubGraph, Nested: SubGraph>(&mut self) {
        if self
            .sub_graphs
            .get(Nested::NAME)
            .is_some_and(|g| g.contains_key(Nested::NAME))
        {
            return;
        }

        self.add_sub_graph::<S>();

        let id = self.sub_graphs.get(S::NAME).unwrap().len();
        let pass = PassBuilder::new(id, Nested::NAME, self).build(SubGraphPass::new::<S>());
        self.sub_graphs
            .get_mut(S::NAME)
            .unwrap()
            .insert(Nested::NAME, pass);
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
            ref_count: u32,
            dependencies: u32,
            dependents: HashSet<usize>,
        }

        impl From<usize> for PassInfo {
            fn from(value: usize) -> Self {
                Self {
                    ref_count: 0,
                    dependencies: value as u32,
                    dependents: HashSet::new(),
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
        let mut pass_infos = graphs
            .iter()
            .map(|(id, passes)| {
                (
                    *id,
                    passes
                        .values()
                        .map(|p| PassInfo::from(p.dependencies.len()))
                        .collect::<Vec<_>>(),
                )
            })
            .collect::<HashMap<_, _>>();

        for (graph, passes) in &graphs {
            let pass_infos = pass_infos.get_mut(graph).unwrap();

            for pass in passes.values() {
                for id in &pass.reads {
                    resource_refs[*id].ref_count += 1;
                    pass_infos[pass.id].ref_count = pass.writes.len() as u32;
                }

                for id in &pass.writes {
                    if resource_refs[*id].first_user.is_none() {
                        resource_refs[*id].first_user = Some(PassId {
                            id: pass.id,
                            graph: *graph,
                        })
                    }
                }

                for dep in &pass.dependencies {
                    let Some(index) = passes.get_index_of(dep) else {
                        continue;
                    };

                    pass_infos[index].dependents.insert(pass.id);
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

            let pass_refs = pass_infos.get_mut(&pass.graph).unwrap();

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
            .map(|(id, passes)| {
                let pass_infos = pass_infos.get_mut(&id).unwrap();
                let mut unreferenced = pass_infos
                    .iter()
                    .enumerate()
                    .filter_map(|(i, info)| (info.ref_count == 0).then_some(i))
                    .collect::<Vec<_>>();

                while let Some(id) = unreferenced.pop() {
                    for dep in std::mem::take(&mut pass_infos[id].dependents) {
                        pass_infos[dep].dependencies -= 1;
                        if pass_infos[dep].dependencies == 0 && pass_infos[dep].ref_count == 0 {
                            unreferenced.push(dep);
                        }
                    }
                }

                let mut graph = IndexDag::new();
                let mut map = HashMap::new();

                let passes = passes
                    .into_values()
                    .filter_map(|p| {
                        if p.has_side_effect
                            || pass_infos[p.id].ref_count > 0
                            || pass_infos[p.id].dependencies > 0
                        {
                            let id = p.id;
                            let node = graph.add_node(p);
                            map.insert(id, node);
                            Some((id, node))
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>();

                for (id, node) in passes {
                    for dep in &pass_infos[id].dependents {
                        let Some(dep) = map.get(dep) else {
                            continue;
                        };

                        graph.add_dependency(node, *dep);
                    }
                }

                (id, graph.build_immutable().unwrap())
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
    pub name: Name,
    reads: HashSet<NodeId>,
    writes: HashSet<NodeId>,
    dependencies: HashSet<Name>,
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
            dependencies: HashSet::new(),
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

    pub fn dependency<P: GraphPass>(&mut self) {
        self.dependencies.insert(P::NAME);
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
            self.dependencies.insert(R::NAME);
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
            dependencies: self.dependencies.drain().collect(),
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
    ) -> impl Fn(&mut RenderContext) -> Result<(), RenderGraphError> + Send + Sync + 'static {
        // Resource is inserted after the pass is created,
        // so we can safely use the builder to get the resource ID.
        let id = builder.builder.resources.len();
        builder.writes.insert(id);

        move |ctx| {
            let resource = &mut ctx.graph.resources[id];
            if resource.object.is_none() {
                resource.create(ctx.world, ctx.device, ctx.surface)
            } else {
                Ok(())
            }
        }
    }
}

pub struct RenderView<'a> {
    pub entity: Entity,
    pub attachments: &'a CameraAttachments,
}

pub struct RenderContext<'a> {
    view: Option<RenderView<'a>>,
    graph: &'a mut RenderGraph,
    world: &'a World,
    device: &'a RenderDevice,
    surface: &'a RenderSurface,
    surface_texture: &'a wgpu::TextureView,
    pipelines: &'a PipelineCache,
    buffers: Vec<wgpu::CommandBuffer>,
}

impl<'a> RenderContext<'a> {
    pub fn new(
        graph: &'a mut RenderGraph,
        world: &'a World,
        device: &'a RenderDevice,
        surface: &'a RenderSurface,
        surface_texture: &'a wgpu::TextureView,
    ) -> Self {
        Self {
            view: None,
            graph,
            world,
            device,
            surface,
            surface_texture,
            pipelines: world.resource::<PipelineCache>(),
            buffers: Vec::new(),
        }
    }

    pub fn view(&self) -> Option<&RenderView<'a>> {
        self.view.as_ref()
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

    pub fn surface_texture(&self) -> &'a wgpu::TextureView {
        self.surface_texture
    }

    pub fn get<R: GraphResource>(&self, id: GraphResourceId<R>) -> Option<&R> {
        self.graph.get::<R>(id)
    }

    pub fn get_render_pipeline(&self, id: &PipelineId) -> Option<&'a RenderPipeline> {
        self.pipelines.get_render_pipeline(id)
    }

    pub fn get_compute_pipeline(&self, id: &PipelineId) -> Option<&'a ComputePipeline> {
        self.pipelines.get_compute_pipeline(id)
    }

    pub fn encoder(&self) -> wgpu::CommandEncoder {
        self.device.create_command_encoder(&Default::default())
    }

    pub fn submit(&mut self, buffer: wgpu::CommandBuffer) {
        self.buffers.push(buffer);
    }

    pub(crate) fn set_view(&mut self, view: Entity, attachments: &'a CameraAttachments) {
        self.view = Some(RenderView {
            entity: view,
            attachments,
        });
    }

    pub(crate) fn run_sub_graph(&mut self, name: Name) -> Result<(), RenderGraphError> {
        let sub_graphs = self.graph.sub_graphs.read_arc_blocking();
        if let Some(passes) = sub_graphs.get(&GraphId::Sub(name)) {
            for pass in passes.iter() {
                pass.execute(self)?;
            }
        }

        Ok(())
    }

    fn run(mut self) -> Result<Vec<wgpu::CommandBuffer>, RenderGraphError> {
        let passes = self.graph.passes.read_arc_blocking();
        for pass in passes.iter() {
            let _ = pass.execute(&mut self);
        }

        Ok(self.buffers)
    }
}

#[derive(Resource)]
pub struct RenderGraph {
    resources: IndexMap<Name, ResourceNode>,
    passes: Arc<RwLock<ImmutableIndexDag<PassNode>>>,
    sub_graphs: Arc<RwLock<HashMap<GraphId, ImmutableIndexDag<PassNode>>>>,
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
        surface_texture: &RenderSurfaceTexture,
    ) {
        let Some(surface_texture) = surface_texture
            .get()
            .map(|v| v.texture.create_view(&Default::default()))
        else {
            return;
        };

        if let Ok(buffers) =
            RenderContext::new(self, world, device, surface, &surface_texture).run()
        {
            device.queue.submit(buffers);
        }
    }

    pub(crate) fn run_graph(
        graph: &mut RenderGraph,
        world: &World,
        device: &RenderDevice,
        surface: &RenderSurface,
        surface_texture: &RenderSurfaceTexture,
    ) {
        graph.run(world, device, surface, surface_texture);
    }

    pub(crate) fn invalidate(
        mut events: Main<EventReader<WindowResized>>,
        graph: &mut RenderGraph,
    ) {
        if events.next().is_some() {
            for resource in graph.resources.values_mut() {
                resource.destroy();
            }
        }
    }

    pub(crate) fn create_graph(mut commands: Commands) {
        commands.add(|world: &mut World| {
            let builder = world
                .remove_resource::<RenderGraphBuilder>()
                .unwrap_or(RenderGraphBuilder::new());

            world.add_resource(builder.build());
        });
    }
}
