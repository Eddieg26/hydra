use crate::{
    core::RenderDevice,
    resources::{BindGroupBuilder, BindGroupLayoutBuilder, TextureDimension},
};
use ecs::{Resource, World};
use std::{
    any::{Any, TypeId},
    cmp,
    collections::{HashMap, HashSet},
    hash::{DefaultHasher, Hash, Hasher},
    marker::PhantomData,
};
use wgpu::{
    BindGroup, BindGroupLayout, ShaderStages, TextureFormat, TextureSampleType, TextureUsages,
    TextureView,
};

pub type Name = &'static str;

pub trait GraphResource: Send + Sync + Sized + 'static {
    type Desc: Send + Sync + Clone + Hash + 'static;

    fn create(device: &RenderDevice, name: Name, desc: &Self::Desc) -> Self;

    fn entry(builder: &mut BindGroupLayoutBuilder, desc: &Self::Desc);

    fn bind<'a>(&'a self, builder: &mut BindGroupBuilder<'a>);
}

#[derive(Clone, Copy, Hash, PartialEq, Eq)]
pub struct TextureDesc {
    dimension: TextureDimension,
    format: TextureFormat,
    usages: TextureUsages,
    sample_type: TextureSampleType,
    sample_count: u32,
}

impl GraphResource for TextureView {
    type Desc = TextureDesc;

    fn create(device: &RenderDevice, name: Name, desc: &Self::Desc) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(name),
            size: desc.dimension.extents(),
            mip_level_count: 1,
            sample_count: desc.sample_count,
            dimension: desc.dimension.into(),
            format: desc.format,
            usage: desc.usages,
            view_formats: &[desc.format.add_srgb_suffix()],
        });

        texture.create_view(&Default::default())
    }

    fn entry(builder: &mut BindGroupLayoutBuilder, desc: &Self::Desc) {
        builder.with_texture(
            ShaderStages::all(),
            desc.sample_type,
            desc.dimension.into(),
            false,
            None,
        );
    }

    fn bind<'a>(&'a self, builder: &mut BindGroupBuilder<'a>) {
        builder.with_texture(self);
    }
}

pub trait ImportedGraphResource: Resource + Send {
    type GraphResource: GraphResource;

    fn resource(&self) -> Self::GraphResource;
}

pub trait GraphPass {
    fn name() -> Name;

    fn setup(builder: &mut PassBuilder) -> impl Fn() + Send + Sync + 'static;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ResourceKind {
    Imported,
    Transient,
}

pub struct ResourceNode {
    id: u32,
    ty: u32,
    name: Name,
    kind: ResourceKind,
    desc: Box<dyn Any>,
    current_version: u32,
    desc_hash: u64,
}

impl ResourceNode {
    pub fn transient<R: GraphResource>(id: u32, ty: u32, name: Name, desc: R::Desc) -> Self {
        let mut hasher = DefaultHasher::new();
        desc.hash(&mut hasher);

        Self {
            id,
            name,
            ty,
            kind: ResourceKind::Transient,
            desc: Box::new(desc),
            current_version: 0,
            desc_hash: hasher.finish(),
        }
    }

    pub fn imported<R: ImportedGraphResource>(id: u32, ty: u32, name: Name) -> Self {
        Self {
            id,
            ty,
            name,
            kind: ResourceKind::Imported,
            desc: Box::new(()),
            current_version: 0,
            desc_hash: 0,
        }
    }
}

pub struct ResourceVersion {
    id: u32,
    node: u32,
    version: u32,
}

impl ResourceVersion {
    pub fn new(id: u32, node: u32, version: u32) -> Self {
        Self { id, node, version }
    }
}

pub struct GpuAllocation {
    id: u32,
    ty: u32,
    generation: u32,
    desc_hash: u64,
    kind: ResourceKind,
    last_use: Option<u32>,
    instance: Box<dyn Any>,
}

impl GpuAllocation {
    pub fn compatible(&self, node: &ResourceNode) -> bool {
        self.desc_hash == node.desc_hash
            && node.kind == ResourceKind::Transient
            && self.kind == ResourceKind::Transient
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GraphVersionId<R: GraphResource>(u32, PhantomData<R>);
impl<R: GraphResource> GraphVersionId<R> {
    fn new(id: u32) -> Self {
        Self(id, PhantomData)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GraphResourceId<R: GraphResource>(u32, PhantomData<R>);
impl<R: GraphResource> GraphResourceId<R> {
    fn new(id: u32) -> Self {
        Self(id, PhantomData)
    }
}

pub struct ResourceType {
    create: fn(&World, &RenderDevice, Name, &Box<dyn Any>) -> Box<dyn Any>,
    entry: fn(&dyn Any, &mut BindGroupLayoutBuilder),
    bind: for<'a> fn(&'a dyn Any, &mut BindGroupBuilder<'a>),
}

impl ResourceType {
    pub fn new<R: GraphResource>() -> Self {
        Self {
            create: |_, device, name, desc| {
                let desc = desc.downcast_ref::<R::Desc>().unwrap();
                Box::new(R::create(device, name, desc))
            },
            entry: |desc, builder| {
                let desc = desc.downcast_ref::<R::Desc>().unwrap();
                R::entry(builder, desc);
            },
            bind: |resource, builder| {
                let resource = resource.downcast_ref::<R>().unwrap();
                R::bind(resource, builder);
            },
        }
    }

    pub fn imported<R: ImportedGraphResource>() -> Self {
        Self {
            create: |world, _, _, _| {
                let imported = world.resource::<R>();
                Box::new(imported.resource())
            },
            entry: |desc, builder| {
                let desc = desc
                    .downcast_ref::<<R::GraphResource as GraphResource>::Desc>()
                    .unwrap();
                <R::GraphResource as GraphResource>::entry(builder, desc);
            },
            bind: |resource, builder| {
                let resource = resource.downcast_ref::<R::GraphResource>().unwrap();
                <R::GraphResource as GraphResource>::bind(resource, builder);
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

    fn entry(&self, desc: &Box<dyn Any>, builder: &mut BindGroupLayoutBuilder) {
        (self.entry)(desc, builder);
    }

    fn bind<'a>(&self, resource: &'a dyn Any, builder: &mut BindGroupBuilder<'a>) {
        (self.bind)(resource, builder);
    }
}

pub struct PassNode {
    id: u32,
    name: Name,
    creates: Vec<u32>,
    reads: Vec<u32>,
    writes: Vec<u32>,
    bindings: Vec<ResourceBinding>,
    execute: Box<dyn Fn() + Send + Sync + 'static>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceUsage {
    Attachment,
    Binding { group: u32, binding: u32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResourceBinding {
    resource: u32,
    group: u32,
    binding: u32,
}

impl Ord for ResourceBinding {
    fn cmp(&self, other: &Self) -> cmp::Ordering {
        match self.group.cmp(&other.group) {
            core::cmp::Ordering::Equal => {}
            ord => return ord,
        }
        self.binding.cmp(&other.binding)
    }
}

impl PartialOrd for ResourceBinding {
    fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
        match self.group.partial_cmp(&other.group) {
            Some(core::cmp::Ordering::Equal) => {}
            ord => return ord,
        }
        self.binding.partial_cmp(&other.binding)
    }
}

pub struct PassBuilder<'a> {
    id: u32,
    creates: Vec<u32>,
    reads: Vec<u32>,
    writes: Vec<u32>,
    bindings: Vec<ResourceBinding>,
    resources: &'a mut RenderGraphResources,
}

impl<'a> PassBuilder<'a> {
    pub fn new(id: u32, resources: &'a mut RenderGraphResources) -> Self {
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
        let id = self.resources.create(name, desc);

        self.creates.push(id.0);

        id
    }

    pub fn import<R: ImportedGraphResource>(
        &mut self,
        name: Name,
    ) -> GraphResourceId<R::GraphResource> {
        let id = self.resources.import::<R>(name);

        id
    }

    pub fn read<R: GraphResource>(
        &mut self,
        resource: GraphResourceId<R>,
        usage: ResourceUsage,
    ) -> GraphVersionId<R> {
        let id = self.resources.versions.len() as u32;
        let version = self.resources.nodes[resource.0 as usize].current_version;

        self.reads.push(id);
        self.resources
            .versions
            .push(ResourceVersion::new(id, version, resource.0));

        if let ResourceUsage::Binding { group, binding } = usage {
            self.bindings.push(ResourceBinding {
                resource: resource.0,
                group,
                binding,
            });
        }

        GraphVersionId::new(id)
    }

    pub fn write<R: GraphResource>(
        &mut self,
        resource: GraphResourceId<R>,
        usage: ResourceUsage,
    ) -> GraphVersionId<R> {
        let id = self.resources.versions.len() as u32;
        let version = self.resources.nodes[resource.0 as usize].current_version + 1;

        self.writes.push(id);
        self.resources.nodes[resource.0 as usize].current_version = version;
        self.resources
            .versions
            .push(ResourceVersion::new(id, version, resource.0));

        if let ResourceUsage::Binding { group, binding } = usage {
            self.bindings.push(ResourceBinding {
                resource: resource.0,
                group,
                binding,
            });
        }

        GraphVersionId::new(id)
    }

    fn build<P: GraphPass>(mut self) -> PassNode {
        let execute = P::setup(&mut self);

        self.bindings.sort();

        PassNode {
            id: self.id,
            name: P::name(),
            creates: self.creates,
            reads: self.reads,
            writes: self.writes,
            bindings: self.bindings,
            execute: Box::new(execute),
        }
    }
}

pub struct CompiledGraph {
    resources: Vec<GpuAllocation>,
    resource_refs: Vec<usize>,
}

pub struct RenderContext<'a> {
    graph: &'a mut RenderGraph,
}

impl<'a> RenderContext<'a> {
    pub fn get<R: GraphResource>(&self, id: GraphVersionId<R>) -> &R {
        let version = &self.graph.resources.versions[id.0 as usize];
        let node = &self.graph.resources.nodes[version.node as usize];
        let resource = self.graph.compiled.resource_refs[node.id as usize];
        self.graph.compiled.resources[resource]
            .instance
            .downcast_ref()
            .expect("Failed to cast resource.")
    }
}

pub struct RenderGraphResources {
    types: Vec<ResourceType>,
    nodes: Vec<ResourceNode>,
    versions: Vec<ResourceVersion>,

    type_map: HashMap<TypeId, u32>,
    node_map: HashMap<Name, u32>,
}

impl RenderGraphResources {
    pub fn new() -> Self {
        Self {
            types: Vec::new(),
            nodes: Vec::new(),
            versions: Vec::new(),
            type_map: HashMap::new(),
            node_map: HashMap::new(),
        }
    }

    pub fn create<R: GraphResource>(&mut self, name: Name, desc: R::Desc) -> GraphResourceId<R> {
        let ty = self.register::<R>();
        if let Some(id) = self.node_map.get(name) {
            GraphResourceId::new(*id)
        } else {
            let id = self.nodes.len() as u32;
            let node = ResourceNode::transient::<R>(id, ty, name, desc);
            self.nodes.push(node);
            self.node_map.insert(name, id);

            GraphResourceId::new(id)
        }
    }

    pub fn import<R: ImportedGraphResource>(
        &mut self,
        name: Name,
    ) -> GraphResourceId<R::GraphResource> {
        let ty = self.register_import::<R>();
        if let Some(id) = self.node_map.get(name) {
            GraphResourceId::new(*id)
        } else {
            let id = self.nodes.len() as u32;
            let node = ResourceNode::imported::<R>(id, ty, name);
            self.nodes.push(node);
            self.node_map.insert(name, id);

            GraphResourceId::new(id)
        }
    }

    fn register<R: GraphResource>(&mut self) -> u32 {
        let ty = TypeId::of::<R>();
        if let Some(index) = self.type_map.get(&ty) {
            return *index as u32;
        } else {
            let index = self.types.len() as u32;
            let resource_ty = ResourceType::new::<R>();
            self.types.push(resource_ty);
            self.type_map.insert(ty, index);
            index
        }
    }

    fn register_import<R: ImportedGraphResource>(&mut self) -> u32 {
        self.register::<R::GraphResource>();

        let ty = TypeId::of::<R>();
        if let Some(index) = self.type_map.get(&ty) {
            return *index as u32;
        } else {
            let index = self.types.len() as u32;
            let resource_ty = ResourceType::imported::<R>();
            self.types.push(resource_ty);
            self.type_map.insert(ty, index);
            index
        }
    }
}

pub struct RenderGraph {
    resources: RenderGraphResources,
    passes: Vec<PassNode>,
    pass_map: HashMap<Name, u32>,
    compiled: CompiledGraph,
}

impl RenderGraph {
    pub fn compile(&self, device: &RenderDevice, world: &World) {
        struct ResourceRef {
            id: u32,
            producer: Option<u32>,
            first_use: Option<u32>,
            last_use: Option<u32>,
        }

        impl From<&ResourceNode> for ResourceRef {
            fn from(value: &ResourceNode) -> Self {
                Self {
                    id: value.id,
                    producer: None,
                    first_use: None,
                    last_use: None,
                }
            }
        }

        struct PassRef<'a> {
            id: u32,
            bindings: &'a [u32],
        }

        impl<'a> From<&'a PassNode> for PassRef<'a> {
            fn from(value: &'a PassNode) -> Self {
                Self {
                    id: value.id,
                    bindings: &[],
                }
            }
        }

        let mut resources = self
            .resources
            .nodes
            .iter()
            .map(ResourceRef::from)
            .collect::<Vec<_>>();

        let mut passes = Vec::with_capacity(self.passes.len());

        for pass in &self.passes {
            for index in &pass.creates {
                let node = self.resources.versions[*index as usize].node;
                resources[node as usize].producer = Some(pass.id);
            }

            for index in &pass.reads {
                let node = self.resources.versions[*index as usize].node;
                resources[node as usize].first_use = Some(pass.id);
                resources[node as usize].last_use = Some(pass.id);
            }

            for index in &pass.writes {
                let node = self.resources.versions[*index as usize].node;
                resources[node as usize].first_use = Some(pass.id);
                resources[node as usize].last_use = Some(pass.id);
            }

            passes.push(PassRef::from(pass));
        }

        resources.sort_by(|a, b| a.first_use.cmp(&b.first_use));

        let mut allocated = Vec::<GpuAllocation>::with_capacity(resources.len());
        let mut refs = Vec::with_capacity(resources.len());

        for resource in resources {
            let node = &self.resources.nodes[resource.id as usize];
            let ty = &self.resources.types[node.ty as usize];
            if let Some(index) = allocated.iter().position(|physical| {
                physical.compatible(node)
                    && resource.first_use.cmp(&physical.last_use) == cmp::Ordering::Greater
            }) {
                refs[resource.id as usize] = index;
                allocated[index].last_use = resource.last_use;
            } else {
                let index = allocated.len();
                allocated.push(GpuAllocation {
                    id: node.id,
                    ty: node.ty,
                    generation: 0,
                    desc_hash: node.desc_hash,
                    kind: node.kind,
                    last_use: resource.last_use,
                    instance: Box::new(ty.create(world, device, node.name, &node.desc)),
                });

                refs[resource.id as usize] = index;
            }
        }
    }
}

pub struct ResourceEntry {
    resource: u32,
    generation: u32,
}

impl ResourceEntry {
    pub fn new(resource: u32, generation: u32) -> Self {
        Self {
            resource,
            generation,
        }
    }
}

pub struct GraphBindGroup {
    layout: BindGroupLayout,
    bind_group: BindGroup,
    entries: Vec<ResourceEntry>,
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct BindGroupId(Box<[u32]>);

impl From<&[ResourceBinding]> for BindGroupId {
    fn from(value: &[ResourceBinding]) -> Self {
        Self(Box::from_iter(value.iter().map(|v| v.resource)))
    }
}

pub struct BindGroupCache {
    bind_groups: Vec<GraphBindGroup>,
    map: HashMap<BindGroupId, u32>,
}
