use crate::{
    core::RenderDevice,
    resources::{BindGroupBuilder, BindGroupLayoutBuilder, GpuResourceId},
};
use ecs::World;
use std::{
    any::{Any, TypeId},
    collections::HashMap,
    hash::{Hash, Hasher},
    marker::PhantomData,
};
use wgpu::ShaderStages;

pub mod allocator;
pub mod compiler;

pub type Name = &'static str;
pub type Execute = Box<dyn Fn() + Send + Sync + 'static>;

pub trait GraphResource: Send + Sync + Sized + 'static {
    type Desc: Hash + Send + Sync + 'static;

    fn create(device: &RenderDevice, name: Name, desc: &Self::Desc) -> Self;

    fn entry(builder: &mut BindGroupLayoutBuilder, desc: &Self::Desc, stages: ShaderStages);

    fn bind<'a>(&'a self, builder: &mut BindGroupBuilder<'a>);
}

pub trait ImportedGraphResource: Send + Sync + Sized + 'static {
    type GraphResource: GraphResource;

    type Desc: Default + Hash + Send + Sync + 'static;

    const ROOT: bool = false;

    fn name() -> Name;

    fn get<'a>(world: &'a World, desc: &'a Self::Desc) -> &'a Self;

    fn resource(&self) -> Self::GraphResource;

    fn generation(&self) -> u32;
}

pub trait GraphPass: Send + Sync + 'static {
    fn name() -> Name;

    fn setup(builder: &mut PassBuilder) -> impl Fn() + Send + Sync + 'static;
}

pub struct SubGraphExpander<'a> {
    graph: &'a RenderGraph,
    versions: &'a mut Vec<ResourceVersion>,
    resources: &'a mut [ResourceRef],
    passes: Vec<PassRef>,
}

impl<'a> SubGraphExpander<'a> {
    pub fn new(
        graph: &'a RenderGraph,
        resources: &'a mut [ResourceRef],
        versions: &'a mut Vec<ResourceVersion>,
    ) -> Self {
        Self {
            graph,
            versions,
            resources,
            passes: Vec::new(),
        }
    }

    pub fn add_resource(&mut self, node: u32, desc: Option<Box<dyn Any>>) -> u32 {
        let id = self.versions.len() as u32;
        let node = &self.graph.resources.nodes[node as usize];
        let resource = ResourceVersion {
            id,
            node: node.id,
            key: node.key,
            producer: None,
            user: None,
            desc,
        };

        self.versions.push(resource);

        id
    }

    pub fn add_pass(
        &mut self,
        node: u32,
        reads: Vec<u32>,
        writes: Vec<u32>,
        bindings: Vec<ResourceBinding>,
    ) -> u32 {
        let id = self.passes.len() as u32;
        let ref_count = writes.len() as u32;

        for index in &writes {
            self.versions[*index as usize].producer = Some(id);
        }

        for index in &reads {
            let node = self.versions[*index as usize].node as usize;
            self.versions[*index as usize].user = Some(id);
            self.resources[node].ref_count += 1;
        }

        self.passes.push(PassRef {
            id,
            node,
            ref_count,
            reads,
            writes,
            bindings,
        });

        id
    }

    fn finish(self) -> Vec<PassRef> {
        self.passes
    }
}

pub trait SubGraph: Send + Sync + 'static {
    fn name() -> Name;

    fn setup(
        id: u32,
        _: &mut PassBuilder,
    ) -> impl Fn(&mut SubGraphExpander) + Send + Sync + 'static;
}

impl<G: SubGraph> GraphPass for G {
    fn name() -> Name {
        <G as SubGraph>::name()
    }

    fn setup(_: &mut PassBuilder) -> impl Fn() + Send + Sync + 'static {
        || {}
    }
}

pub struct ResourceType {
    create: fn(&World, &RenderDevice, Name, &dyn Any) -> Box<dyn Any>,
    entry: fn(&dyn Any, &mut BindGroupLayoutBuilder, ShaderStages),
    bind: for<'a> fn(&'a dyn Any, &mut BindGroupBuilder<'a>),
    generation: fn(&World, &dyn Any) -> u32,
}

impl ResourceType {
    fn transient<R: GraphResource>() -> Self {
        Self {
            create: |_, device, name, desc| {
                let desc = desc.downcast_ref::<R::Desc>().unwrap();
                Box::new(R::create(device, name, desc))
            },
            entry: |desc, builder, stages| {
                let desc = desc.downcast_ref::<R::Desc>().unwrap();
                R::entry(builder, desc, stages);
            },
            bind: |resource, builder| {
                let resource = resource.downcast_ref::<R>().unwrap();
                R::bind(resource, builder);
            },
            generation: |_, _| 0,
        }
    }

    fn imported<R: ImportedGraphResource>() -> Self {
        Self {
            create: |world: &World, _, _, desc| {
                let desc = desc.downcast_ref::<R::Desc>().unwrap();
                let imported = R::get(world, desc);
                Box::new(imported.resource())
            },
            entry: |desc, builder, stages| {
                let desc = desc
                    .downcast_ref::<<R::GraphResource as GraphResource>::Desc>()
                    .unwrap();
                <R::GraphResource as GraphResource>::entry(builder, desc, stages);
            },
            bind: |resource, builder| {
                let resource = resource.downcast_ref::<R::GraphResource>().unwrap();
                <R::GraphResource as GraphResource>::bind(resource, builder);
            },
            generation: |world, desc| {
                let desc = desc.downcast_ref::<R::Desc>().unwrap();
                R::get(world, desc).generation()
            },
        }
    }

    fn create(
        &self,
        world: &World,
        device: &RenderDevice,
        name: Name,
        desc: &Box<dyn Any>,
    ) -> Box<dyn Any> {
        (self.create)(world, device, name, desc)
    }

    fn entry(
        &self,
        desc: &Box<dyn Any>,
        builder: &mut BindGroupLayoutBuilder,
        stages: ShaderStages,
    ) {
        (self.entry)(desc, builder, stages);
    }

    fn bind<'a>(&self, resource: &'a dyn Any, builder: &mut BindGroupBuilder<'a>) {
        (self.bind)(resource, builder);
    }

    fn generation(&self, world: &World, desc: &dyn Any) -> u32 {
        (self.generation)(world, desc)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ResourceKind {
    Transient,
    Imported,
}

pub struct ResourceNode {
    id: u32,
    ty: u32,
    name: Name,
    root: bool,
    kind: ResourceKind,
    key: u64,
    desc: Box<dyn Any>,
}

impl ResourceNode {
    fn transient<R: GraphResource>(id: u32, ty: u32, name: Name, desc: R::Desc) -> Self {
        Self {
            id,
            ty,
            name,
            root: false,
            kind: ResourceKind::Transient,
            key: Self::key(&desc),
            desc: Box::new(desc),
        }
    }

    fn imported<R: ImportedGraphResource>(id: u32, ty: u32, name: Name) -> Self {
        let desc = R::Desc::default();
        Self {
            id,
            ty,
            name,
            root: R::ROOT,
            kind: ResourceKind::Imported,
            key: Self::key(&desc),
            desc: Box::new(desc),
        }
    }

    fn key<H: Hash>(value: &H) -> u64 {
        let mut state = std::hash::DefaultHasher::new();
        value.hash(&mut state);
        state.finish()
    }
}

pub struct ResourceVersion {
    id: u32,
    node: u32,
    key: u64,
    producer: Option<u32>,
    user: Option<u32>,
    desc: Option<Box<dyn Any>>,
}

#[derive(Debug, Default)]
pub struct ResourceRef {
    node: u32,
    ref_count: u32,
}

impl From<&ResourceNode> for ResourceRef {
    fn from(value: &ResourceNode) -> Self {
        Self {
            node: value.id,
            ref_count: match value.root {
                true => 1,
                false => 0,
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ResourceUsage {
    Attachment,
    Binding {
        group: u32,
        binding: u32,
        stages: ShaderStages,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResourceBinding {
    resource: u32,
    group: u32,
    binding: u32,
    stages: ShaderStages,
}

impl Ord for ResourceBinding {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        match self.group.cmp(&other.group) {
            core::cmp::Ordering::Equal => {}
            ord => return ord,
        }
        self.binding.cmp(&other.binding)
    }
}

impl PartialOrd for ResourceBinding {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        match self.group.partial_cmp(&other.group) {
            Some(core::cmp::Ordering::Equal) => {}
            ord => return ord,
        }
        self.binding.partial_cmp(&other.binding)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PassAttachment {
    Attachment {
        resource: u32,
    },
    Binding {
        resource: u32,
        group: u32,
        binding: u32,
        stages: ShaderStages,
    },
}

impl PassAttachment {
    pub fn resource(&self) -> u32 {
        match self {
            PassAttachment::Attachment { resource } => *resource,
            PassAttachment::Binding { resource, .. } => *resource,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GraphResourceId<R: GraphResource>(u32, PhantomData<R>);
impl<R: GraphResource> GraphResourceId<R> {
    fn new(id: u32) -> Self {
        Self(id, PhantomData)
    }
}

pub struct PassNode {
    id: u32,
    name: Name,
    creates: Vec<u32>,
    reads: Vec<PassAttachment>,
    writes: Vec<PassAttachment>,
    execute: Execute,
    expand: Box<dyn Fn(&mut SubGraphExpander) + Send + Sync + 'static>,
}

impl PassNode {
    fn expand(&self, expander: &mut SubGraphExpander) {
        (self.expand)(expander);
    }
}

pub struct PassRef {
    id: u32,
    node: u32,
    ref_count: u32,
    reads: Vec<u32>,
    writes: Vec<u32>,
    bindings: Vec<ResourceBinding>,
}

pub struct SubGraphNode {
    id: u32,
    passes: Vec<u32>,
}

impl SubGraphNode {
    fn new(id: u32) -> Self {
        Self {
            id,
            passes: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GraphName {
    Main,
    SubGraph(Name),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PassKey {
    name: Name,
    graph: GraphName,
}

impl PassKey {
    pub fn new(name: Name, graph: GraphName) -> Self {
        Self { name, graph }
    }
}

pub struct PassBuilder<'a> {
    id: u32,
    creates: Vec<u32>,
    reads: Vec<PassAttachment>,
    writes: Vec<PassAttachment>,
    bindings: Vec<ResourceBinding>,
    resources: &'a mut GraphResources,
}

impl<'a> PassBuilder<'a> {
    pub fn new(id: u32, resources: &'a mut GraphResources) -> Self {
        Self {
            id,
            resources,
            creates: Vec::new(),
            reads: Vec::new(),
            writes: Vec::new(),
            bindings: Vec::new(),
        }
    }

    pub fn get<R: GraphResource>(&mut self, name: Name) -> GraphResourceId<R> {
        let index = self
            .resources
            .node_map
            .get(name)
            .expect("Resource has not been created or imported.");
        GraphResourceId::new(*index)
    }

    pub fn create<R: GraphResource>(&mut self, name: Name, desc: R::Desc) -> GraphResourceId<R> {
        let id = self.resources.create::<R>(name, desc);

        self.creates.push(id);

        GraphResourceId::new(id)
    }

    pub fn import<R: ImportedGraphResource>(&mut self) -> GraphResourceId<R::GraphResource> {
        let id = self.resources.import::<R>();

        GraphResourceId::new(id)
    }

    pub fn read<R: GraphResource>(
        &mut self,
        resource: GraphResourceId<R>,
        usage: ResourceUsage,
    ) -> GraphResourceId<R> {
        let attachment = match usage {
            ResourceUsage::Attachment => PassAttachment::Attachment {
                resource: resource.0,
            },
            ResourceUsage::Binding {
                group,
                binding,
                stages,
            } => PassAttachment::Binding {
                resource: resource.0,
                group,
                binding,
                stages,
            },
        };

        self.reads.push(attachment);

        resource
    }

    pub fn write<R: GraphResource>(
        &mut self,
        resource: GraphResourceId<R>,
        usage: ResourceUsage,
    ) -> GraphResourceId<R> {
        let attachment = match usage {
            ResourceUsage::Attachment => PassAttachment::Attachment {
                resource: resource.0,
            },
            ResourceUsage::Binding {
                group,
                binding,
                stages,
            } => PassAttachment::Binding {
                resource: resource.0,
                group,
                binding,
                stages,
            },
        };

        self.writes.push(attachment);

        resource
    }

    fn build<P: GraphPass>(
        mut self,
        expand: impl Fn(&mut SubGraphExpander) + Send + Sync + 'static,
    ) -> PassNode {
        let id = self.id;
        let execute = P::setup(&mut self);

        self.bindings.sort();

        PassNode {
            id,
            name: P::name(),
            creates: self.creates,
            reads: self.reads,
            writes: self.writes,
            execute: Box::new(execute),
            expand: Box::new(expand),
        }
    }
}

#[derive(Default)]
pub struct GraphResources {
    types: Vec<ResourceType>,
    nodes: Vec<ResourceNode>,

    type_map: HashMap<TypeId, u32>,
    node_map: HashMap<Name, u32>,
}

impl GraphResources {
    pub fn create<R: GraphResource>(&mut self, name: Name, desc: R::Desc) -> u32 {
        if let Some(id) = self.node_map.get(name) {
            *id
        } else {
            let id = self.nodes.len() as u32;
            let ty = self.register::<R>();
            self.nodes
                .push(ResourceNode::transient::<R>(id, ty, name, desc));
            self.node_map.insert(name, id);

            id
        }
    }

    pub fn import<R: ImportedGraphResource>(&mut self) -> u32 {
        let name = R::name();
        if let Some(id) = self.node_map.get(name) {
            *id
        } else {
            let id = self.nodes.len() as u32;
            let ty = self.register_import::<R>();
            self.nodes.push(ResourceNode::imported::<R>(id, ty, name));
            self.node_map.insert(name, id);

            id
        }
    }

    fn register<R: GraphResource>(&mut self) -> u32 {
        let ty = TypeId::of::<R>();
        if let Some(id) = self.type_map.get(&ty) {
            *id
        } else {
            let id = self.types.len() as u32;
            self.types.push(ResourceType::transient::<R>());
            self.type_map.insert(ty, id);

            id
        }
    }

    fn register_import<R: ImportedGraphResource>(&mut self) -> u32 {
        let ty = TypeId::of::<R>();
        if let Some(id) = self.type_map.get(&ty) {
            *id
        } else {
            let id = self.types.len() as u32;
            self.types.push(ResourceType::imported::<R>());
            self.type_map.insert(ty, id);

            id
        }
    }
}

pub struct GraphPasses {
    nodes: Vec<PassNode>,
    graphs: Vec<SubGraphNode>,

    node_map: HashMap<PassKey, u32>,
    graph_map: HashMap<GraphName, u32>,
}

pub struct RenderGraph {
    passes: GraphPasses,
    resources: GraphResources,
}

impl RenderGraph {
    pub const MAIN_GRAPH: usize = 0;

    pub fn add_pass<P: GraphPass>(&mut self) {
        let key = PassKey::new(P::name(), GraphName::Main);
        if !self.passes.node_map.contains_key(&key) {
            let id = self.passes.nodes.len() as u32;
            let pass = PassBuilder::new(id, &mut self.resources).build::<P>(|_| {});
            self.passes.nodes.push(pass);
            self.passes.node_map.insert(key, id);
        }
    }

    pub fn add_subgraph<G: SubGraph>(&mut self) {
        let name = GraphName::SubGraph(G::name());
        if !self.passes.graph_map.contains_key(&name) {
            let id = self.passes.graphs.len() as u32;
            let pass_id = self.passes.nodes.len() as u32;

            let mut builder = PassBuilder::new(pass_id, &mut self.resources);
            let expand = G::setup(id, &mut builder);

            self.passes.graphs[Self::MAIN_GRAPH].passes.push(pass_id);
            self.passes.nodes.push(builder.build::<G>(expand));
            self.passes.graphs.push(SubGraphNode::new(id));
            self.passes.graph_map.insert(name, id);
        }
    }

    pub fn create<R: GraphResource>(&mut self, name: Name, desc: R::Desc) -> GpuResourceId<R> {
        let id = self.resources.create::<R>(name, desc);

        GpuResourceId::new(id)
    }

    pub fn import<R: ImportedGraphResource>(&mut self) -> GpuResourceId<R> {
        let id = self.resources.import::<R>();

        GpuResourceId::new(id)
    }
}
