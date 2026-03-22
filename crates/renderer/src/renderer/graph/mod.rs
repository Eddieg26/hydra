use crate::{
    core::{RenderDevice, RenderSettings},
    renderer::{
        camera::{CameraQueue, CameraSettings},
        graph::allocator::{GpuResourceAllocator, PassBindGroup},
    },
    resources::{BindGroupBuilder, BindGroupLayoutBuilder},
};
use ecs::{Condition, Resource, World};
use std::{
    any::{Any, TypeId},
    collections::HashMap,
    marker::PhantomData,
};
use wgpu::{BindGroup, CommandBuffer, CommandEncoder, Extent3d, ShaderStages};

pub mod allocator;
pub mod compiler;

pub type Name = &'static str;
pub type DynData = dyn Any + Send + Sync + 'static;
pub type BoxData = Box<DynData>;

pub struct ResourceResolver<'a> {
    resource: u32,
    world: &'a World,
    camera: Option<&'a CameraSettings>,
    textures: HashMap<u32, Extent3d>,
}

impl<'a> ResourceResolver<'a> {
    pub fn new(world: &'a World, camera: Option<&'a CameraSettings>) -> Self {
        Self {
            resource: 0,
            world,
            camera,
            textures: HashMap::new(),
        }
    }

    pub fn world(&self) -> &World {
        self.world
    }

    pub fn camera(&self) -> Option<&CameraSettings> {
        self.camera
    }

    pub fn texture(&self, id: u32) -> Option<Extent3d> {
        self.textures.get(&id).copied()
    }

    pub fn add_texture(&mut self, width: u32, height: u32, depths_or_layers: u32) {
        self.textures.insert(
            self.resource,
            Extent3d {
                width,
                height,
                depth_or_array_layers: depths_or_layers,
            },
        );
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ResourceKind {
    Transient,
    Imported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceUsage {
    Attachment,
    Binding {
        group: u32,
        binding: u32,
        visiblitiy: ShaderStages,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceAccess {
    Read,
    Write,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResourceBinding {
    entry: u32,
    group: u32,
    binding: u32,
    visiblitiy: ShaderStages,
    access: ResourceAccess,
}

impl Ord for ResourceBinding {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.group
            .cmp(&other.group)
            .then_with(|| self.binding.cmp(&other.binding))
    }
}

impl PartialOrd for ResourceBinding {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

pub trait GraphResource: Send + Sync + Sized + 'static {
    type Desc: Clone + Send + Sync + Sized + 'static;

    fn resolve(
        world: &ecs::World,
        settings: &crate::core::RenderSettings,
        resolver: &mut ResourceResolver,
        desc: Self::Desc,
    ) -> Self::Desc;

    fn create(device: &RenderDevice, name: Name, desc: &Self::Desc) -> Self;

    fn entry(
        settings: &RenderSettings,
        desc: &Self::Desc,
        builder: &mut BindGroupLayoutBuilder,
        visibility: ShaderStages,
    );

    fn bind<'a>(&'a self, builder: &mut BindGroupBuilder<'a>);

    fn compatible(desc_a: &Self::Desc, desc_b: &Self::Desc) -> bool;

    fn generation(&self) -> u32;

    fn kind() -> ResourceKind;
}

pub struct PassBuilder<'a> {
    resources: &'a mut GraphResources,
    refs: u32,
    creates: Vec<u32>,
    entries: Vec<u32>,
    bindings: Vec<ResourceBinding>,
}

impl<'a> PassBuilder<'a> {
    pub fn new(resources: &'a mut GraphResources) -> Self {
        Self {
            resources,
            refs: 0,
            creates: Vec::new(),
            entries: Vec::new(),
            bindings: Vec::new(),
        }
    }

    pub fn create<R: GraphResource>(&mut self, name: Name, desc: R::Desc) -> GraphResourceId<R> {
        let id = self.resources.create(name, desc);
        self.creates.push(id.0);
        id
    }

    pub fn read<R: GraphResource>(
        &mut self,
        resource: GraphResourceId<R>,
        usage: ResourceUsage,
    ) -> GraphEntryId<R> {
        let id = self.add_entry(resource, usage, ResourceAccess::Read);
        self.entries.push(id.0);

        id
    }

    pub fn write<R: GraphResource>(
        &mut self,
        resource: GraphResourceId<R>,
        usage: ResourceUsage,
    ) -> GraphEntryId<R> {
        let id = self.add_entry(resource, usage, ResourceAccess::Write);
        self.entries.push(id.0);
        self.refs += 1;

        id
    }

    fn add_entry<R: GraphResource>(
        &mut self,
        resource: GraphResourceId<R>,
        usage: ResourceUsage,
        access: ResourceAccess,
    ) -> GraphEntryId<R> {
        let id = self.resources.entries.len() as u32;
        self.resources.entries.push(ResourceEntry {
            id,
            node: resource.0,
            access,
        });

        if let ResourceUsage::Binding {
            group,
            binding,
            visiblitiy,
        } = usage
        {
            self.bindings.push(ResourceBinding {
                entry: id,
                group,
                binding,
                visiblitiy,
                access,
            });
        }

        GraphEntryId::new(id)
    }

    fn build<P: GraphPass>(mut self, id: u32) -> PassNode {
        let execute = P::setup(&mut self);

        self.bindings.sort();

        PassNode {
            id,
            name: P::NAME,
            refs: self.refs,
            creates: self.creates.into_boxed_slice(),
            entries: self.entries.into_boxed_slice(),
            bindings: self.bindings.into_boxed_slice(),
            execute: Box::new(execute),
        }
    }
}

pub trait GraphPass: Send + Sync + Sized + 'static {
    const NAME: Name;

    fn setup(builder: &mut PassBuilder) -> impl Fn(&mut RenderContext) + Send + Sync + 'static;
}

pub struct ResourceType {
    resolve: fn(&World, &RenderSettings, &mut ResourceResolver, BoxData) -> BoxData,
    create: fn(&RenderDevice, Name, &dyn Any) -> BoxData,
    entry: fn(&RenderSettings, &DynData, &mut BindGroupLayoutBuilder, ShaderStages),
    bind: for<'a> fn(&'a DynData, &mut BindGroupBuilder<'a>),
    clone: fn(&DynData) -> BoxData,
    compatible: fn(&DynData, &DynData) -> bool,
    generation: fn(&DynData) -> u32,
}

impl ResourceType {
    fn new<R: GraphResource>() -> Self {
        Self {
            resolve: |world, settings, resolver, desc| {
                let desc = *desc.downcast::<R::Desc>().unwrap();
                Box::new(R::resolve(world, settings, resolver, desc))
            },
            create: |device, name, desc| {
                let desc = desc.downcast_ref::<R::Desc>().unwrap();
                Box::new(R::create(device, name, desc))
            },
            entry: |settings, desc, builder, visibility| {
                let desc = desc.downcast_ref::<R::Desc>().unwrap();
                R::entry(settings, desc, builder, visibility);
            },
            bind: |resource, builder| {
                let resource = resource.downcast_ref::<R>().unwrap();
                R::bind(resource, builder);
            },
            clone: |desc| {
                let desc = desc.downcast_ref::<R::Desc>().unwrap();
                Box::new(desc.clone())
            },
            compatible: |desc_a, desc_b| {
                let desc_a = desc_a.downcast_ref::<R::Desc>().unwrap();
                let desc_b = desc_b.downcast_ref::<R::Desc>().unwrap();

                R::compatible(desc_a, desc_b)
            },
            generation: |resource| resource.downcast_ref::<R>().unwrap().generation(),
        }
    }

    pub fn resolve(
        &self,
        world: &World,
        settings: &RenderSettings,
        resolver: &mut ResourceResolver,
        desc: BoxData,
    ) -> BoxData {
        (self.resolve)(world, settings, resolver, desc)
    }

    pub fn create(&self, device: &RenderDevice, name: Name, desc: &DynData) -> BoxData {
        (self.create)(device, name, desc)
    }

    pub fn entry(
        &self,
        settings: &RenderSettings,
        desc: &DynData,
        builder: &mut BindGroupLayoutBuilder,
        visibility: ShaderStages,
    ) {
        (self.entry)(settings, desc, builder, visibility);
    }

    pub fn bind<'a>(&self, resource: &'a DynData, builder: &mut BindGroupBuilder<'a>) {
        (self.bind)(resource, builder)
    }

    pub fn clone(&self, desc: &DynData) -> BoxData {
        (self.clone)(desc)
    }

    pub fn compatible(&self, desc_a: &DynData, desc_b: &DynData) -> bool {
        (self.compatible)(desc_a, desc_b)
    }

    pub fn generation(&self, resource: &DynData) -> u32 {
        (self.generation)(resource)
    }
}

pub struct ResourceNode {
    id: u32,
    ty: u32,
    name: Name,
    kind: ResourceKind,
    desc: BoxData,
}

impl ResourceNode {
    fn new<R: GraphResource>(
        id: u32,
        ty: u32,
        name: Name,
        kind: ResourceKind,
        desc: R::Desc,
    ) -> Self {
        Self {
            id,
            ty,
            name,
            kind,
            desc: Box::new(desc),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResourceEntry {
    id: u32,
    node: u32,
    access: ResourceAccess,
}

pub struct GraphResourceId<R: GraphResource>(u32, PhantomData<R>);
impl<R: GraphResource> GraphResourceId<R> {
    fn new(id: u32) -> Self {
        Self(id, PhantomData)
    }

    pub fn get(&self) -> u32 {
        self.0
    }
}

pub struct GraphEntryId<R: GraphResource>(u32, PhantomData<R>);
impl<R: GraphResource> GraphEntryId<R> {
    fn new(id: u32) -> Self {
        Self(id, PhantomData)
    }

    pub fn offset(&self, offset: u32) -> Self {
        Self::new(self.0 + offset)
    }

    pub fn get(&self) -> u32 {
        self.0
    }
}

pub struct GraphResources {
    types: Vec<ResourceType>,
    nodes: Vec<ResourceNode>,
    entries: Vec<ResourceEntry>,

    type_map: HashMap<TypeId, usize>,
    node_map: HashMap<Name, usize>,
}

impl GraphResources {
    pub fn new() -> Self {
        Self {
            types: Vec::new(),
            nodes: Vec::new(),
            entries: Vec::new(),
            type_map: HashMap::new(),
            node_map: HashMap::new(),
        }
    }

    pub fn create<R: GraphResource>(&mut self, name: Name, desc: R::Desc) -> GraphResourceId<R> {
        let ty = self.register::<R>() as u32;
        let kind = R::kind();
        let id = if let Some(index) = self.node_map.get(name).copied() {
            self.nodes[index] = ResourceNode::new::<R>(index as u32, ty, name, kind, desc);
            index
        } else {
            let id = self.nodes.len();
            let node = ResourceNode::new::<R>(id as u32, ty, name, kind, desc);
            self.nodes.push(node);
            id
        };

        self.node_map.insert(name, id);
        GraphResourceId::new(id as u32)
    }

    fn register<R: GraphResource>(&mut self) -> usize {
        let ty = TypeId::of::<R>();
        if let Some(index) = self.type_map.get(&ty).copied() {
            index
        } else {
            let index = self.types.len();
            self.types.push(ResourceType::new::<R>());
            self.type_map.insert(ty, index);
            index
        }
    }
}

pub struct PassNode {
    id: u32,
    name: Name,
    refs: u32,
    creates: Box<[u32]>,
    entries: Box<[u32]>,
    bindings: Box<[ResourceBinding]>,
    execute: Box<dyn Fn(&mut RenderContext) + Send + Sync + 'static>,
}

impl PassNode {
    pub fn id(&self) -> u32 {
        self.id
    }

    pub fn name(&self) -> Name {
        self.name
    }

    pub fn creates(&self) -> &[u32] {
        &self.creates
    }

    pub fn entries(&self) -> &[u32] {
        &self.entries
    }

    pub fn bindings(&self) -> &[ResourceBinding] {
        &self.bindings
    }

    pub fn execute(&self, ctx: &mut RenderContext) {
        (self.execute)(ctx);
    }
}

pub struct RenderContext<'a> {
    resource_offset: u32,
    world: &'a World,
    device: &'a RenderDevice,
    allocator: &'a GpuResourceAllocator,
    buffers: Vec<CommandBuffer>,
}

impl<'a> RenderContext<'a> {
    pub fn new(
        world: &'a World,
        device: &'a RenderDevice,
        allocator: &'a GpuResourceAllocator,
    ) -> Self {
        Self {
            resource_offset: 0,
            world,
            device,
            allocator,
            buffers: Vec::new(),
        }
    }

    pub fn device(&self) -> &RenderDevice {
        self.device
    }

    pub fn world(&self) -> &World {
        self.world
    }

    pub fn get<R: GraphResource>(&self, id: GraphEntryId<R>) -> &R {
        let id = id.offset(self.resource_offset);
        self.allocator.resource(id)
    }

    pub fn bind_group(&self, index: u32) -> &BindGroup {
        self.allocator.bind_groups().get(index)
    }

    pub fn encoder(&self) -> CommandEncoder {
        self.device.create_command_encoder(&Default::default())
    }

    pub fn submit(&mut self, encoder: CommandEncoder) {
        self.buffers.push(encoder.finish());
    }
}

pub struct RenderGraphState {
    cameras: CameraQueue,
    settings: RenderSettings,
    passes: Box<[PassInstance]>,
    allocator: GpuResourceAllocator,
}

#[derive(Default, Resource)]
pub struct ExecutableGraph(Option<RenderGraphState>);

impl ExecutableGraph {
    pub(crate) fn set(
        &mut self,
        cameras: CameraQueue,
        settings: RenderSettings,
        passes: Box<[PassInstance]>,
        allocator: GpuResourceAllocator,
    ) {
        self.0 = Some(RenderGraphState {
            cameras,
            settings,
            passes,
            allocator,
        })
    }

    pub fn state(&self) -> Option<&RenderGraphState> {
        self.0.as_ref()
    }

    pub fn clear(&mut self) {
        self.0 = None;
    }
}

pub struct PassInstance {
    pub node: u32,
    pub resources: u32,
    pub bindings: Box<[PassBindGroup]>,
}

#[derive(Resource)]
pub struct RenderGraph {
    resources: GraphResources,
    nodes: Vec<PassNode>,
    node_map: HashMap<TypeId, usize>,
}

impl RenderGraph {
    pub fn add_pass<P: GraphPass>(&mut self) {
        let ty = TypeId::of::<P>();

        if !self.node_map.contains_key(&ty) {
            let id = self.nodes.len();
            let node = PassBuilder::new(&mut self.resources).build::<P>(id as u32);

            self.nodes.push(node);
            self.node_map.insert(ty, id);
        }
    }

    pub fn resources(&self) -> &GraphResources {
        &self.resources
    }

    pub fn run(
        world: &World,
        device: &RenderDevice,
        graph: &RenderGraph,
        executable: &mut ExecutableGraph,
    ) {
        if let Some(state) = executable.0.as_mut() {
            state.allocator.update(device, graph);

            let mut ctx = RenderContext::new(world, device, &state.allocator);
            for pass in &state.passes {
                ctx.resource_offset = pass.resources;

                let node = &graph.nodes[pass.node as usize];
                node.execute(&mut ctx);
            }

            device.queue().submit(ctx.buffers);
        }
    }
}

pub struct RenderGraphDirty;
impl Condition for RenderGraphDirty {
    fn evaluate(world: &World, _: &ecs::SystemMeta) -> bool {
        let graph = world.resource::<ExecutableGraph>();
        let settings = world.resource::<RenderSettings>();
        let cameras = world.resource::<CameraQueue>();
        !graph
            .state()
            .is_some_and(|state| &state.settings == settings && &state.cameras != cameras)
    }
}

pub struct RecompileRenderGraph;
impl ecs::Command for RecompileRenderGraph {
    fn execute(self, world: &mut World) {
        world.resource_mut::<ExecutableGraph>().clear();
    }
}

pub mod test {
    use crate::{
        core::{ColorFormat, Msaa, RenderSettings},
        renderer::{camera::SettingState, graph::GraphResource},
        resources::MainRenderTarget,
    };
    use wgpu::TextureDescriptor;

    pub struct SurfaceInput {
        pub color: wgpu::TextureView,
        pub msaa_color: Option<wgpu::TextureView>,
    }

    #[derive(Clone, PartialEq, Eq)]
    pub enum SurfaceInputDesc {
        Auto,
        Fixed {
            width: u32,
            height: u32,
            color: ColorFormat,
            msaa: Msaa,
        },
    }

    impl GraphResource for SurfaceInput {
        type Desc = SurfaceInputDesc;

        fn resolve(
            world: &ecs::World,
            settings: &crate::core::RenderSettings,
            resolver: &mut super::ResourceResolver,
            desc: Self::Desc,
        ) -> Self::Desc {
            if let SurfaceInputDesc::Fixed { .. } = &desc {
                return desc;
            }

            let (width, height, color, msaa) = match resolver.camera.as_ref() {
                Some(camera) => {
                    let msaa = match camera.msaa {
                        SettingState::Auto => settings.msaa(),
                        SettingState::Disabled => Msaa::Disabled,
                    };

                    (camera.width, camera.height, camera.format, msaa)
                }
                None => {
                    let main = world
                        .resource::<MainRenderTarget>()
                        .get()
                        .expect("Main Render Target not set");

                    (main.width, main.height, main.format, settings.msaa())
                }
            };

            resolver.add_texture(width, height, 1);

            SurfaceInputDesc::Fixed {
                width,
                height,
                color,
                msaa,
            }
        }

        fn create(
            device: &crate::core::RenderDevice,
            name: super::Name,
            desc: &Self::Desc,
        ) -> Self {
            let SurfaceInputDesc::Fixed {
                width,
                height,
                color,
                msaa,
            } = desc
            else {
                panic!("Suface Input Desc not resolved.")
            };

            let size = wgpu::Extent3d {
                width: *width,
                height: *height,
                depth_or_array_layers: 1,
            };

            let color = device.create_texture(&TextureDescriptor {
                label: Some(name),
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: (*color).into(),
                usage: wgpu::TextureUsages::all(),
                view_formats: &[],
            });

            let msaa = match *msaa {
                crate::core::Msaa::Disabled => None,
                _ => Some(device.create_texture(&TextureDescriptor {
                    label: Some(&format!("{name}_MSAA")),
                    size,
                    mip_level_count: 1,
                    sample_count: msaa.sample_count(),
                    dimension: wgpu::TextureDimension::D2,
                    format: color.format(),
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                    view_formats: &[],
                })),
            };

            Self {
                color: color.create_view(&Default::default()),
                msaa_color: msaa.map(|msaa| msaa.create_view(&Default::default())),
            }
        }

        fn entry(
            _: &RenderSettings,
            _: &Self::Desc,
            builder: &mut crate::resources::BindGroupLayoutBuilder,
            visibility: wgpu::ShaderStages,
        ) {
            builder.with_texture(
                visibility,
                wgpu::TextureSampleType::Float { filterable: true },
                wgpu::TextureViewDimension::D2,
                false,
                None,
            );
        }

        fn bind<'a>(&'a self, builder: &mut crate::resources::BindGroupBuilder<'a>) {
            builder.with_texture(&self.color);
        }

        fn compatible(desc_a: &Self::Desc, desc_b: &Self::Desc) -> bool {
            desc_a == desc_b
        }

        fn generation(&self) -> u32 {
            0
        }

        fn kind() -> super::ResourceKind {
            super::ResourceKind::Transient
        }
    }
}
