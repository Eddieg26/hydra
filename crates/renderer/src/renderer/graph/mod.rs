use crate::{
    core::{RenderDevice, RenderSettings},
    renderer::{
        camera::{CameraQueue, CameraSettings},
        graph::{
            allocator::{BindGroupCache, GpuResourceAllocator, PassBindGroup},
            compiler::{CompiledRenderGraph, RenderGraphCompiler},
        },
    },
    resources::{BindGroupBuilder, BindGroupLayoutBuilder, BindGroupLayoutRegistry},
};
use ecs::{Component, Condition, FixedBitSet, Resource, World};
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
    Create,
    Read,
    Write,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResourceBinding {
    node: u32,
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
    entries: Vec<ResourceEntry>,
    bindings: Vec<ResourceBinding>,
}

impl<'a> PassBuilder<'a> {
    pub fn new(resources: &'a mut GraphResources) -> Self {
        Self {
            resources,
            entries: Vec::new(),
            bindings: Vec::new(),
        }
    }

    pub fn create<R: GraphResource>(&mut self, name: Name, desc: R::Desc) -> GraphResourceId<R> {
        let id = self.resources.create(name, desc);
        self.entries.push(ResourceEntry {
            node: id.0,
            access: ResourceAccess::Create,
        });

        id
    }

    pub fn read<R: GraphResource>(
        &mut self,
        resource: GraphResourceId<R>,
        usage: ResourceUsage,
    ) -> GraphResourceId<R> {
        self.entries.push(ResourceEntry {
            node: resource.0,
            access: ResourceAccess::Read,
        });

        if let ResourceUsage::Binding {
            group,
            binding,
            visiblitiy,
        } = usage
        {
            self.bindings.push(ResourceBinding {
                node: resource.0,
                group,
                binding,
                visiblitiy,
                access: ResourceAccess::Read,
            });
        }

        resource
    }

    pub fn write<R: GraphResource>(
        &mut self,
        resource: GraphResourceId<R>,
        usage: ResourceUsage,
    ) -> GraphResourceId<R> {
        self.entries.push(ResourceEntry {
            node: resource.0,
            access: ResourceAccess::Write,
        });

        if let ResourceUsage::Binding {
            group,
            binding,
            visiblitiy,
        } = usage
        {
            self.bindings.push(ResourceBinding {
                node: resource.0,
                group,
                binding,
                visiblitiy,
                access: ResourceAccess::Write,
            });
        }

        resource
    }

    fn build<P: GraphPass>(mut self, id: u32) -> PassNode {
        let execute = P::setup(&mut self);

        self.bindings.sort();

        PassNode {
            id: PassId(id),
            name: P::NAME,
            entries: self.entries.into_boxed_slice(),
            bindings: self.bindings.into_boxed_slice(),
            dependencies: RenderGraphMask::new(id as usize),
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

    pub fn offset(&self, offset: u32) -> Self {
        Self(self.0 + offset, PhantomData)
    }
}

impl<R: GraphResource> Copy for GraphResourceId<R> {}
impl<R: GraphResource> Clone for GraphResourceId<R> {
    fn clone(&self) -> Self {
        Self(self.0.clone(), self.1.clone())
    }
}

pub struct GraphResources {
    types: Vec<ResourceType>,
    nodes: Vec<ResourceNode>,

    type_map: HashMap<TypeId, usize>,
    node_map: HashMap<Name, usize>,
}

impl GraphResources {
    pub fn new() -> Self {
        Self {
            types: Vec::new(),
            nodes: Vec::new(),
            type_map: HashMap::new(),
            node_map: HashMap::new(),
        }
    }

    pub fn ty(&self, ty: u32) -> &ResourceType {
        &self.types[ty as usize]
    }

    pub fn node(&self, id: u32) -> &ResourceNode {
        &self.nodes[id as usize]
    }

    pub fn nodes(&self) -> &[ResourceNode] {
        &self.nodes
    }

    fn create<R: GraphResource>(&mut self, name: Name, desc: R::Desc) -> GraphResourceId<R> {
        let ty = self.register::<R>() as u32;
        let kind = R::kind();
        let id = self.nodes.len();
        let node = ResourceNode::new::<R>(id as u32, ty, name, kind, desc);

        self.nodes.push(node);
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct PassId(u32);
impl std::ops::Deref for PassId {
    type Target = u32;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub struct PassNode {
    id: PassId,
    name: Name,
    entries: Box<[ResourceEntry]>,
    bindings: Box<[ResourceBinding]>,
    dependencies: RenderGraphMask,
    execute: Box<dyn Fn(&mut RenderContext) + Send + Sync + 'static>,
}

impl PassNode {
    pub fn id(&self) -> PassId {
        self.id
    }

    pub fn name(&self) -> Name {
        self.name
    }

    pub fn entries(&self) -> &[ResourceEntry] {
        &self.entries
    }

    pub fn bindings(&self) -> &[ResourceBinding] {
        &self.bindings
    }

    pub fn dependencies(&self) -> &RenderGraphMask {
        &self.dependencies
    }

    pub fn execute(&self, ctx: &mut RenderContext) {
        (self.execute)(ctx);
    }
}

pub struct RenderContext<'a> {
    resource_offset: u32,
    world: &'a World,
    device: &'a RenderDevice,
    state: &'a RenderGraphState,
    camera: Option<&'a CameraSettings>,
    buffers: Vec<CommandBuffer>,
}

impl<'a> RenderContext<'a> {
    pub fn new(world: &'a World, device: &'a RenderDevice, state: &'a RenderGraphState) -> Self {
        Self {
            resource_offset: 0,
            world,
            device,
            state,
            camera: None,
            buffers: Vec::new(),
        }
    }

    pub fn device(&self) -> &RenderDevice {
        self.device
    }

    pub fn world(&self) -> &World {
        self.world
    }

    pub fn settings(&self) -> &RenderSettings {
        &self.state.settings
    }

    pub fn camera(&self) -> Option<&CameraSettings> {
        self.camera
    }

    pub fn get<R: GraphResource>(&self, id: GraphResourceId<R>) -> &R {
        let id = id.offset(self.resource_offset);
        self.state.allocator.resource(id)
    }

    pub fn bind_group(&self, index: u32) -> &BindGroup {
        self.state.allocator.bind_groups().get(index)
    }

    pub fn encoder(&self, label: impl Into<Option<&'static str>>) -> CommandEncoder {
        self.device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: label.into(),
            })
    }

    pub fn submit(&mut self, encoder: CommandEncoder) {
        self.buffers.push(encoder.finish());
    }
}

pub struct PassInstance {
    pub node: u32,
    pub cursor: u32,
    pub camera: Option<u32>,
    pub bindings: Box<[PassBindGroup]>,
}

#[derive(Clone, Component, PartialEq, Eq, Hash)]
pub struct RenderGraphMask(FixedBitSet);
impl RenderGraphMask {
    pub fn new(capacity: usize) -> Self {
        Self(FixedBitSet::with_capacity(capacity))
    }

    pub fn get(&self, pass: PassId) -> bool {
        self.0.contains(*pass as usize)
    }

    pub fn set(&mut self, pass: PassId, enabled: bool) {
        self.0.grow(pass.0 as usize);
        self.0.set(pass.0 as usize, enabled);
    }
}

pub struct RenderGraphState {
    cameras: CameraQueue,
    settings: RenderSettings,
    passes: Box<[PassInstance]>,
    allocator: GpuResourceAllocator,
}

impl RenderGraphState {
    pub fn new(
        cameras: CameraQueue,
        settings: RenderSettings,
        passes: Box<[PassInstance]>,
        allocator: GpuResourceAllocator,
    ) -> Self {
        Self {
            cameras,
            settings,
            passes,
            allocator,
        }
    }
}

#[derive(Resource)]
pub struct RenderGraph {
    resources: GraphResources,
    nodes: Vec<PassNode>,
    node_map: HashMap<TypeId, usize>,
    state: Option<RenderGraphState>,
}

impl RenderGraph {
    pub fn add_pass<P: GraphPass>(&mut self) -> PassId {
        let ty = TypeId::of::<P>();

        if let Some(id) = self.node_map.get(&ty) {
            PassId(*id as u32)
        } else {
            let id = self.nodes.len();
            let node = PassBuilder::new(&mut self.resources).build::<P>(id as u32);

            self.nodes.push(node);
            self.node_map.insert(ty, id);
            PassId(id as u32)
        }
    }

    pub fn add_before<P: GraphPass>(&mut self, target: PassId) -> PassId {
        let id = self.add_pass::<P>();

        let target = &mut self.nodes[target.0 as usize];
        target.dependencies.set(id, true);

        id
    }

    pub fn add_after<P: GraphPass>(&mut self, target: PassId) -> PassId {
        let id = self.add_pass::<P>();

        let pass = &mut self.nodes[id.0 as usize];
        pass.dependencies.set(target, true);

        id
    }

    pub fn resources(&self) -> &GraphResources {
        &self.resources
    }

    pub fn state(&self) -> Option<&RenderGraphState> {
        self.state.as_ref()
    }

    pub(crate) fn update(
        world: &World,
        device: &RenderDevice,
        settings: &RenderSettings,
        cameras: &CameraQueue,
        graph: &mut RenderGraph,
        layouts: &mut BindGroupLayoutRegistry,
    ) {
        let CompiledRenderGraph {
            passes,
            resources,
            allocations,
            bind_group_layouts,
            bind_groups,
        } = RenderGraphCompiler::run(world, graph, settings, cameras);

        let layouts = BindGroupCache::create_layouts(device, layouts, bind_group_layouts);

        let allocator = GpuResourceAllocator::build(
            device,
            graph,
            resources,
            allocations,
            layouts,
            bind_groups,
        );

        let state = RenderGraphState::new(cameras.clone(), *settings, passes, allocator);
        graph.state = Some(state);
    }

    pub(crate) fn run(world: &World, device: &RenderDevice, graph: &mut RenderGraph) {
        let RenderGraph {
            resources,
            nodes,
            state,
            ..
        } = graph;

        if let Some(state) = state {
            state.allocator.update(device, resources);

            let mut ctx = RenderContext::new(world, device, state);
            for pass in &state.passes {
                ctx.camera = pass.camera.map(|i| &state.cameras.slice()[i as usize]);
                ctx.resource_offset = pass.cursor;

                let node = &nodes[pass.node as usize];
                node.execute(&mut ctx);
            }

            device.queue().submit(ctx.buffers);
        }
    }
}

pub struct RenderGraphDirty;
impl Condition for RenderGraphDirty {
    fn evaluate(world: &World, _: &ecs::SystemMeta) -> bool {
        let graph = world.resource::<RenderGraph>();
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
        world.resource_mut::<RenderGraph>().state = None;
    }
}

pub mod test {
    use crate::{
        core::{ColorFormat, DepthFormat, Msaa, RenderSettings},
        renderer::{
            camera::SettingState,
            graph::{GraphPass, GraphResource, ResourceUsage},
        },
        resources::{MainRenderTarget, PipelineId, RenderAssets, RenderTarget},
    };
    use wgpu::{ShaderStages, TextureDescriptor};

    #[derive(Clone, Copy, PartialEq, Eq)]
    pub enum SurfaceKind {
        Color(ColorFormat),
        Depth(DepthFormat),
    }

    #[derive(Clone, Copy, PartialEq, Eq)]
    pub enum SurfaceSize {
        Auto,
        Fixed { width: u32, height: u32 },
    }

    #[derive(Clone, Copy, PartialEq, Eq)]
    pub struct SurfaceDesc {
        pub size: SurfaceSize,
        pub kind: SurfaceKind,
        pub msaa: Msaa,
    }

    pub struct SurfaceTexture {
        pub view: wgpu::TextureView,
        pub msaa_view: Option<wgpu::TextureView>,
    }

    impl GraphResource for SurfaceTexture {
        type Desc = SurfaceDesc;

        fn resolve(
            world: &ecs::World,
            settings: &crate::core::RenderSettings,
            resolver: &mut super::ResourceResolver,
            desc: Self::Desc,
        ) -> Self::Desc {
            let kind = match desc.kind {
                SurfaceKind::Color(_) => SurfaceKind::Color(settings.color()),
                SurfaceKind::Depth(_) => SurfaceKind::Depth(settings.depth()),
            };

            let (width, height, msaa) = match desc.size {
                SurfaceSize::Auto => match resolver.camera.as_ref() {
                    Some(camera) => {
                        let msaa = match camera.msaa {
                            SettingState::Auto => settings.msaa(),
                            SettingState::Disabled => Msaa::Disabled,
                        };

                        (camera.width, camera.height, msaa)
                    }
                    None => {
                        let main = world
                            .resource::<MainRenderTarget>()
                            .get()
                            .expect("Main Render Target not set");

                        (main.width, main.height, settings.msaa())
                    }
                },
                SurfaceSize::Fixed { width, height } => (width, height, desc.msaa),
            };

            resolver.add_texture(width, height, 1);

            SurfaceDesc {
                size: SurfaceSize::Fixed { width, height },
                kind,
                msaa,
            }
        }

        fn create(
            device: &crate::core::RenderDevice,
            name: super::Name,
            desc: &Self::Desc,
        ) -> Self {
            let SurfaceSize::Fixed { width, height } = desc.size else {
                panic!("Suface Input Desc not resolved.")
            };

            let format = match desc.kind {
                SurfaceKind::Color(format) => wgpu::TextureFormat::from(format),
                SurfaceKind::Depth(format) => wgpu::TextureFormat::from(format),
            };

            let size = wgpu::Extent3d {
                width: width,
                height: height,
                depth_or_array_layers: 1,
            };

            let texture = device.create_texture(&TextureDescriptor {
                label: Some(name),
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage: wgpu::TextureUsages::all(),
                view_formats: &[],
            });

            let msaa = match desc.msaa {
                crate::core::Msaa::Disabled => None,
                _ => Some(device.create_texture(&TextureDescriptor {
                    label: Some(&format!("{name}_MSAA")),
                    size,
                    mip_level_count: 1,
                    sample_count: desc.msaa.sample_count(),
                    dimension: wgpu::TextureDimension::D2,
                    format,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                    view_formats: &[],
                })),
            };

            Self {
                view: texture.create_view(&Default::default()),
                msaa_view: msaa.map(|msaa| msaa.create_view(&Default::default())),
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
            builder.with_texture(&self.view);
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

    pub struct RenderOutput {
        view: wgpu::TextureView,
        generation: u32,
    }

    #[derive(Clone, PartialEq, Eq)]
    pub enum RenderOutputDesc {
        Auto,
        Resolved {
            view: wgpu::TextureView,
            generation: u32,
        },
    }

    impl GraphResource for RenderOutput {
        type Desc = RenderOutputDesc;

        fn resolve(
            world: &ecs::World,
            _: &crate::core::RenderSettings,
            resolver: &mut super::ResourceResolver,
            desc: Self::Desc,
        ) -> Self::Desc {
            if let RenderOutputDesc::Resolved { .. } = &desc {
                return desc;
            };

            let targets = world.resource::<RenderAssets<RenderTarget>>();
            let target = match resolver.camera() {
                Some(camera) => camera.target.and_then(|id| targets.get(&id)),
                None => world.resource::<MainRenderTarget>().get(),
            };

            let Some(target) = target else {
                panic!("Render target not found.")
            };

            let view = target.color.view().clone();
            let generation = resolver.camera.map(|c| c.generation).unwrap_or(0);

            RenderOutputDesc::Resolved { view, generation }
        }

        fn create(_: &crate::core::RenderDevice, _: super::Name, desc: &Self::Desc) -> Self {
            let RenderOutputDesc::Resolved { view, generation } = desc else {
                panic!("Render output not resolved.")
            };

            Self {
                view: view.clone(),
                generation: *generation,
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
            builder.with_texture(&self.view);
        }

        fn compatible(desc_a: &Self::Desc, desc_b: &Self::Desc) -> bool {
            desc_a == desc_b
        }

        fn generation(&self) -> u32 {
            self.generation
        }

        fn kind() -> super::ResourceKind {
            super::ResourceKind::Imported
        }
    }

    pub struct OutputPass;

    impl OutputPass {
        fn copy(
            ctx: &super::RenderContext,
            src: &wgpu::TextureView,
            dst: &wgpu::TextureView,
        ) -> wgpu::CommandEncoder {
            let mut encoder = ctx.encoder("output_copy");

            {
                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("output_copy"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: dst,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });

                // set render pipeline
                pass.set_bind_group(0, ctx.bind_group(0), &[]);
                pass.draw(0..3, 0..1);
            }

            encoder
        }

        fn tonemap(
            ctx: &super::RenderContext,
            src: &wgpu::TextureView,
            dst: &wgpu::TextureView,
            srgb: bool,
        ) -> wgpu::CommandEncoder {
            let pipeline: PipelineId = match srgb {
                true => todo!(),
                false => todo!(),
            };

            let mut encoder = ctx.encoder("output_tonemap");

            {
                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("output_tonemap"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: dst,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });

                pass.set_bind_group(0, ctx.bind_group(0), &[]);
                pass.draw(0..3, 0..1);
            }

            encoder
        }
    }

    impl GraphPass for OutputPass {
        const NAME: super::Name = "OutputPass";

        fn setup(
            builder: &mut super::PassBuilder,
        ) -> impl Fn(&mut super::RenderContext) + Send + Sync + 'static {
            let color = builder.create::<SurfaceTexture>(
                "intermediate_color",
                SurfaceDesc {
                    size: SurfaceSize::Auto,
                    kind: SurfaceKind::Color(ColorFormat::HDR),
                    msaa: Msaa::Disabled,
                },
            );
            let output = builder.create::<RenderOutput>("output", RenderOutputDesc::Auto);

            let src = builder.read(
                color,
                ResourceUsage::Binding {
                    group: 0,
                    binding: 0,
                    visiblitiy: ShaderStages::FRAGMENT,
                },
            );
            let dst = builder.write(output, ResourceUsage::Attachment);

            move |ctx: &mut super::RenderContext<'_>| {
                let camera = ctx.camera().expect("Missing camera for OutputPass");
                let src = ctx.get(src);
                let dst = ctx.get(dst);

                let encoder = match (ctx.settings().color(), camera.format) {
                    (ColorFormat::Standard { .. }, ColorFormat::Standard { .. }) => {
                        OutputPass::copy(ctx, &src.view, &dst.view)
                    }
                    (ColorFormat::HDR, ColorFormat::HDR) => {
                        OutputPass::copy(ctx, &src.view, &dst.view)
                    }
                    (ColorFormat::HDR, ColorFormat::Standard { srgb }) => {
                        OutputPass::tonemap(ctx, &src.view, &dst.view, srgb)
                    }
                    (ColorFormat::Standard { .. }, ColorFormat::HDR) => unreachable!(),
                };

                ctx.submit(encoder);
            }
        }
    }
}
