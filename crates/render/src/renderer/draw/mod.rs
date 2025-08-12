use crate::{
    Aabb, ActiveCamera, ArrayBuffer, AsBinding, BindGroup, BindGroupBuilder, BindGroupLayout,
    BindGroupLayoutBuilder, Buffer, CameraAttachments, FragmentState, Mesh, MeshFilter, MeshFormat,
    MeshKey, MeshLayout, PipelineCache, PipelineId, RenderAsset, RenderAssets,
    RenderCommandEncoder, RenderDevice, RenderMesh, RenderPipelineDesc, RenderState, RenderSurface,
    Shader, SubMesh, VertexState, allocator::MeshAllocator, uniform::UniformBufferArray,
};
use asset::{Asset, AssetId, ErasedId};
use ecs::{
    AddComponent, ArgItem, Commands, Component, Entity, IndexMap, Phase, Query, ReadOnly, Resource,
    SystemArg, SystemMeta, World, WorldAccess,
    commands::AddResource,
    query::{Single, With, Without},
    unlifetime::Read,
    world::WorldCell,
};
use encase::ShaderType;
use math::{Mat4, Size};
use std::{any::TypeId, marker::PhantomData, ops::Range};
use transform::{GlobalTransform, LocalTransform};
use wgpu::{
    BufferUsages, ColorTargetState, DynamicOffset, Operations, PrimitiveState,
    RenderPassColorAttachment, RenderPassDepthStencilAttachment, RenderPassDescriptor,
    ShaderStages, VertexFormat, VertexStepMode,
    wgt::{DrawIndexedIndirectArgs, DrawIndirectArgs},
};

#[derive(Debug, Clone, Copy, ShaderType)]
pub struct RenderView {
    pub world: Mat4,
    pub view: Mat4,
    pub projection: Mat4,
}

#[derive(Debug, Clone, Copy, Component)]
pub struct ViewInstance<V: View> {
    pub offset: DynamicOffset,
    _marker: PhantomData<V>,
}

pub trait View: Clone + Component {
    type Transform: LocalTransform;

    fn projection(&self, width: f32, height: f32) -> Mat4;
}

#[derive(Resource)]
pub struct ViewBuffer<V: View> {
    buffer: UniformBufferArray<RenderView>,
    layout: BindGroupLayout,
    bind_group: BindGroup,
    _marker: PhantomData<V>,
}

impl<V: View> ViewBuffer<V> {
    pub fn new(device: &RenderDevice) -> Self {
        let buffer = UniformBufferArray::new(device, None, None);
        let layout = BindGroupLayoutBuilder::new()
            .with_uniform(0, ShaderStages::all(), true, None, None)
            .build(device);

        let bind_group = BindGroupBuilder::new(&layout)
            .with_uniform(0, buffer.as_ref(), 0, None)
            .build(device);

        Self {
            buffer,
            layout,
            bind_group,
            _marker: PhantomData,
        }
    }

    pub fn buffer(&self) -> &Buffer {
        self.buffer.as_ref()
    }

    pub fn layout(&self) -> &BindGroupLayout {
        &self.layout
    }

    pub fn bind_group(&self) -> &BindGroup {
        &self.bind_group
    }

    pub fn push(&mut self, view: &RenderView) -> DynamicOffset {
        let offset = self.buffer.push(view);
        offset
    }

    pub fn update(views: &mut Self, device: &RenderDevice) {
        if views.buffer.update(device).is_some() {
            views.bind_group = BindGroupBuilder::new(&views.layout)
                .with_uniform(0, views.buffer.as_ref(), 0, None)
                .build(device);
        }
    }

    pub fn clear(views: &mut Self) {
        views.buffer.clear();
    }

    pub(crate) fn queue(
        surface: &RenderSurface,
        views: Query<(&GlobalTransform, &V, &mut ViewInstance<V>)>,
        new_views: Query<(Entity, &GlobalTransform, &V), Without<ViewInstance<V>>>,
        buffer: &mut ViewBuffer<V>,
        mut commands: Commands,
    ) {
        let Size { width, height } = surface.size();
        for (transform, view, instance) in views {
            let view = RenderView {
                world: transform.matrix(),
                view: transform.view_matrix(),
                projection: view.projection(width as f32, height as f32),
            };

            instance.offset = buffer.push(&view)
        }

        for (entity, transform, view) in new_views.iter() {
            let view = RenderView {
                world: transform.matrix(),
                view: transform.view_matrix(),
                projection: view.projection(width as f32, height as f32),
            };

            let instance = ViewInstance::<V> {
                offset: buffer.push(&view),
                _marker: PhantomData,
            };

            commands.add(AddComponent::new(entity, instance));
        }
    }
}

pub type MaterialId = ErasedId;
pub type ObjectTransform = Mat4;

#[derive(Resource)]
pub struct ObjectBuffer {
    objects: Vec<ObjectTransform>,
    visible: VisibleBuffer,
    buffer: Buffer,
    layout: BindGroupLayout,
    bind_group: BindGroup,
}

impl ObjectBuffer {
    pub fn new(device: &RenderDevice) -> Self {
        let buffer = Buffer::new(
            device,
            ObjectTransform::min_size().get(),
            BufferUsages::STORAGE | BufferUsages::COPY_DST,
            None,
        );

        let visibility = VisibleBuffer::new(device);

        let layout = BindGroupLayoutBuilder::new()
            .with_storage(
                0,
                ShaderStages::VERTEX | ShaderStages::COMPUTE,
                false,
                true,
                None,
                None,
            )
            .with_storage(
                1,
                ShaderStages::VERTEX | ShaderStages::COMPUTE,
                false,
                true,
                None,
                None,
            )
            .build(device);

        let bind_group = BindGroupBuilder::new(&layout)
            .with_storage(0, &buffer, 0, None)
            .with_storage(1, &visibility.indices, 0, None)
            .build(device);

        Self {
            objects: Vec::new(),
            visible: visibility,
            buffer,
            layout,
            bind_group,
        }
    }

    pub fn objects(&self) -> &[ObjectTransform] {
        &self.objects
    }

    pub fn buffer(&self) -> &Buffer {
        &self.buffer
    }

    pub fn layout(&self) -> &BindGroupLayout {
        &self.layout
    }

    pub fn bind_group(&self) -> &BindGroup {
        &self.bind_group
    }

    pub fn push(&mut self, object: ObjectTransform) -> u32 {
        let index = self.objects.len() as u32;
        self.objects.push(object);
        index
    }

    pub fn push_batch(&mut self, objects: Vec<ObjectTransform>) -> Range<u32> {
        if objects.is_empty() {
            return 0..0;
        }

        let instances = self.objects.len() as u32..self.objects.len() as u32 + objects.len() as u32;
        self.objects.extend(objects);
        instances
    }

    pub fn update(objects: &mut Self, device: &RenderDevice) {
        let data = bytemuck::cast_slice(&objects.objects);

        if objects.buffer.size() < data.len() as u64 {
            let len = (device.limits().max_storage_buffer_binding_size as usize).min(data.len());
            objects.buffer.resize_with_data(device, &data[..len]);
            objects.bind_group = BindGroupBuilder::new(&objects.layout)
                .with_storage(0, &objects.buffer, 0, None)
                .build(device);
        } else {
            objects.buffer.update(device, data);
        }

        let size_required = (objects.objects.len() * std::mem::size_of::<u32>()) as u64;
        if objects.visible.indices.size() < size_required {
            objects.visible.indices.resize(device, size_required);
        }

        objects
            .visible
            .count
            .update(device, bytemuck::bytes_of(&0u32));
    }

    pub fn clear(objects: &mut Self) {
        objects.objects.clear();
    }
}

pub struct VisibleBuffer {
    pub indices: Buffer,
    pub count: Buffer,
}

impl VisibleBuffer {
    pub fn new(device: &RenderDevice) -> Self {
        let indices = Buffer::with_data(
            device,
            bytemuck::bytes_of(&0u32),
            BufferUsages::STORAGE,
            None,
        );
        let count = Buffer::with_data(
            device,
            bytemuck::bytes_of(&0u32),
            BufferUsages::STORAGE | BufferUsages::COPY_DST,
            None,
        );

        Self { indices, count }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlendMode {
    Opaque,
    Transparent,
}

impl Into<wgpu::BlendState> for BlendMode {
    fn into(self) -> wgpu::BlendState {
        match self {
            BlendMode::Opaque => wgpu::BlendState::REPLACE,
            BlendMode::Transparent => wgpu::BlendState::ALPHA_BLENDING,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum DepthWrite {
    Auto,
    On,
    Off,
}

pub trait RenderPhase: Send + 'static {
    type View: View;

    fn mode() -> BlendMode;
}

pub trait ShaderModel: Resource + Send + 'static {
    fn create(world: &World) -> Self;

    fn layout(&self) -> Option<&BindGroupLayout> {
        None
    }

    fn bind_group(&self) -> Option<&BindGroup> {
        None
    }
}

#[derive(Resource)]
pub struct Unlit;

impl ShaderModel for Unlit {
    fn create(_: &World) -> Self {
        Unlit
    }
}

pub trait Material: Asset + AsBinding + Clone {
    type View: View;

    type Phase: RenderPhase<View = Self::View>;

    type Model: ShaderModel;

    fn depth_write() -> DepthWrite {
        DepthWrite::Auto
    }

    fn shader() -> impl Into<AssetId<Shader>>;
}

#[derive(Resource)]
pub struct MaterialLayout<M: Material>(BindGroupLayout, PhantomData<M>);
impl<M: Material> MaterialLayout<M> {
    pub fn layout(&self) -> &BindGroupLayout {
        &self.0
    }
}

impl<M: Material> std::ops::Deref for MaterialLayout<M> {
    type Target = BindGroupLayout;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<M: Material> MaterialLayout<M> {
    pub fn new(device: &RenderDevice) -> Self {
        Self(M::create_bind_group_layout(device), PhantomData)
    }
}

pub struct MaterialInstance<M: Material>(BindGroup, PhantomData<M>);
impl<M: Material> std::ops::Deref for MaterialInstance<M> {
    type Target = BindGroup;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<M: Material> RenderAsset for MaterialInstance<M> {
    type Source = M;

    type Arg = (Read<RenderDevice>, Read<MaterialLayout<M>>, M::Arg);

    fn extract(
        _: AssetId<Self::Source>,
        asset: Self::Source,
        (device, layout, arg): &mut ArgItem<Self::Arg>,
    ) -> Result<Self, crate::ExtractError<Self::Source>> {
        let bind_group = asset
            .create_bind_group(device, layout, arg)
            .map_err(|_| crate::ExtractError::Retry(asset))?;

        Ok(Self(bind_group, PhantomData))
    }
}

pub trait Drawable: Clone + Component {
    type View: View;

    type Material: Material<View = Self::View>;

    fn material(&self) -> AssetId<Self::Material>;

    fn primitive() -> PrimitiveState {
        PrimitiveState::default()
    }

    fn vertex() -> &'static [VertexFormat];

    fn shader() -> impl Into<AssetId<Shader>>;
}

pub type DrawPhase<D> = <<D as Drawable>::Material as Material>::Phase;
pub type DrawModel<D> = <<D as Drawable>::Material as Material>::Model;

#[derive(Debug, Clone, Copy, thiserror::Error)]
pub enum DrawError {
    #[error("Skipped draw call")]
    Skip,
}

pub trait DrawCommand: 'static {
    type Arg: ReadOnly;

    fn execute(
        state: &mut RenderState,
        view: DynamicOffset,
        call: &DrawCall,
        arg: ArgItem<Self::Arg>,
    ) -> Result<(), DrawError>;
}

pub struct SetView<V: View, const GROUP: u32>(PhantomData<V>);

pub struct SetPipeline;
impl DrawCommand for SetPipeline {
    type Arg = Read<PipelineCache>;

    fn execute(
        state: &mut RenderState,
        _: DynamicOffset,
        call: &DrawCall,
        pipelines: ArgItem<Self::Arg>,
    ) -> Result<(), DrawError> {
        let pipeline = pipelines
            .get_render_pipeline(&call.pipeline)
            .ok_or(DrawError::Skip)?;

        Ok(state.set_pipeline(pipeline))
    }
}

impl<V: View, const GROUP: u32> DrawCommand for SetView<V, GROUP> {
    type Arg = Read<ViewBuffer<V>>;

    fn execute(
        state: &mut RenderState,
        view: DynamicOffset,
        _: &DrawCall,
        views: ArgItem<Self::Arg>,
    ) -> Result<(), DrawError> {
        Ok(state.set_bind_group(GROUP, views.bind_group(), &[view]))
    }
}

pub struct SetObject<const GROUP: u32>;
impl<const GROUP: u32> DrawCommand for SetObject<GROUP> {
    type Arg = Read<ObjectBuffer>;

    fn execute(
        state: &mut RenderState,
        _: DynamicOffset,
        _: &DrawCall,
        objects: ArgItem<Self::Arg>,
    ) -> Result<(), DrawError> {
        Ok(state.set_bind_group(GROUP, objects.bind_group(), &[]))
    }
}

pub struct SetMaterial<M: Material, const GROUP: u32>(PhantomData<M>);
impl<M: Material, const GROUP: u32> DrawCommand for SetMaterial<M, GROUP> {
    type Arg = Read<RenderAssets<MaterialInstance<M>>>;

    fn execute(
        state: &mut RenderState,
        _: DynamicOffset,
        call: &DrawCall,
        materials: ArgItem<Self::Arg>,
    ) -> Result<(), DrawError> {
        let material = materials
            .get(&call.material.into())
            .ok_or(DrawError::Skip)?;

        Ok(state.set_bind_group(GROUP, &material, &[]))
    }
}

pub struct SetShaderModel<M: ShaderModel, const GROUP: u32>(PhantomData<M>);
impl<M: ShaderModel, const GROUP: u32> DrawCommand for SetShaderModel<M, GROUP> {
    type Arg = Read<M>;

    fn execute(
        state: &mut RenderState,
        _: DynamicOffset,
        _: &DrawCall,
        model: ArgItem<Self::Arg>,
    ) -> Result<(), DrawError> {
        if let Some(bind_group) = model.bind_group() {
            state.set_bind_group(GROUP, bind_group, &[]);
        }

        Ok(())
    }
}

pub struct ExecuteDraw;
impl DrawCommand for ExecuteDraw {
    type Arg = (Read<MeshAllocator>, Read<DrawArgs>);

    fn execute(
        state: &mut RenderState,
        _: DynamicOffset,
        call: &DrawCall,
        (meshes, args): ArgItem<Self::Arg>,
    ) -> Result<(), DrawError> {
        let vertex = meshes.vertex_slice(&call.mesh).ok_or(DrawError::Skip)?;

        match call.format {
            MeshFormat::NonIndexed => {
                state.set_vertex_buffer(0, vertex.buffer.slice(..));
                Ok(state.draw_indirect(args.non_indexed.as_ref(), call.offset))
            }
            MeshFormat::Indexed { format, .. } => {
                let index = meshes.index_slice(&call.mesh).ok_or(DrawError::Skip)?;

                state.set_vertex_buffer(0, vertex.buffer.slice(..));
                state.set_index_buffer(index.buffer.slice(..), format);
                Ok(state.draw_indexed_indirect(args.indexed.as_ref(), call.offset))
            }
        }
    }
}

pub type Draw<D> = (
    SetPipeline,
    SetView<<D as Drawable>::View, 0>,
    SetObject<1>,
    SetMaterial<<D as Drawable>::Material, 2>,
    SetShaderModel<DrawModel<D>, 3>,
    ExecuteDraw,
);

pub type DrawFunction =
    Box<dyn FnMut(&mut RenderState, DynamicOffset, &DrawCall, &SystemMeta, WorldCell) + Send>;

#[derive(Default, Resource)]
pub struct DrawCommands {
    functions: IndexMap<TypeId, DrawFunction>,
    access: WorldAccess,
}

impl DrawCommands {
    pub fn add<R: DrawCommand>(&mut self, world: &mut World) -> usize {
        let ty = TypeId::of::<R>();
        if let Some(index) = self.functions.get_index_of(&ty) {
            return index;
        };

        let mut state = <R::Arg as SystemArg>::init(world, &mut self.access);
        let f = move |render: &mut RenderState,
                      view: DynamicOffset,
                      call: &DrawCall,
                      system: &SystemMeta,
                      world: WorldCell| {
            let arg = unsafe { <R::Arg as SystemArg>::get(&mut state, world, &system) };
            let _ = R::execute(render, view, call, arg);
        };

        let index = self.functions.len();
        self.functions.insert(ty, Box::new(f));

        index
    }

    pub fn get(&self, function: DrawFunctionId) -> &DrawFunction {
        &self.functions[function.0]
    }

    pub fn get_mut(&mut self, function: DrawFunctionId) -> &mut DrawFunction {
        &mut self.functions[function.0]
    }
}

pub struct DrawFunctions<'world> {
    world: WorldCell<'world>,
    commands: &'world mut DrawCommands,
}

impl<'world> DrawFunctions<'world> {
    pub fn draw(
        &mut self,
        state: &mut RenderState,
        view: DynamicOffset,
        call: &DrawCall,
        meta: &SystemMeta,
    ) {
        let function = self.commands.get_mut(call.function);
        function(state, view, call, meta, self.world);
    }
}

impl std::ops::Deref for DrawFunctions<'_> {
    type Target = DrawCommands;

    fn deref(&self) -> &Self::Target {
        self.commands
    }
}

unsafe impl SystemArg for DrawFunctions<'_> {
    type Item<'world, 'state> = DrawFunctions<'world>;

    type State = ();

    fn init(world: &mut World, access: &mut WorldAccess) -> Self::State {
        let other = std::mem::take(&mut world.resource_mut::<DrawCommands>().access);
        access.extend(other);
    }

    unsafe fn get<'world, 'state>(
        _: &'state mut Self::State,
        mut world: ecs::world::WorldCell<'world>,
        _: &'world SystemMeta,
    ) -> Self::Item<'world, 'state> {
        let commands = unsafe { world.get_mut() }.resource_mut::<DrawCommands>();

        DrawFunctions { world, commands }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DrawFunctionId(usize);

pub struct DrawId<D: Drawable>(pub DrawFunctionId, PhantomData<D>);

impl<D: Drawable> Copy for DrawId<D> {}
impl<D: Drawable> Clone for DrawId<D> {
    fn clone(&self) -> Self {
        Self(self.0.clone(), self.1.clone())
    }
}

unsafe impl<D: Drawable> SystemArg for DrawId<D> {
    type Item<'world, 'state> = Self;

    type State = Self;

    fn init(world: &mut World, _: &mut WorldAccess) -> Self::State {
        let mut commands = world.remove_resource::<DrawCommands>().unwrap_or_default();
        let id = commands.add::<Draw<D>>(world);
        world.add_resource(commands);

        DrawId::<D>(DrawFunctionId(id), PhantomData)
    }

    unsafe fn get<'world, 'state>(
        state: &'state mut Self::State,
        _: WorldCell<'world>,
        _: &'world SystemMeta,
    ) -> Self::Item<'world, 'state> {
        *state
    }
}

#[derive(Resource)]
pub struct DrawPipeline<D: Drawable> {
    pub id: PipelineId,
    pub key: MeshKey,
    _marker: PhantomData<D>,
}

impl<D: Drawable> DrawPipeline<D> {
    pub fn queue(
        surface: &RenderSurface,
        views: &ViewBuffer<D::View>,
        objects: &ObjectBuffer,
        material: &MaterialLayout<D::Material>,
        model: &DrawModel<D>,
        pipelines: &mut PipelineCache,
        mut commands: Commands,
    ) {
        let vertex_shader: AssetId<Shader> = D::shader().into();
        let fragment_shader: AssetId<Shader> = D::Material::shader().into();

        let buffers = vec![MeshLayout::into_vertex_buffer_layout(
            0,
            D::vertex(),
            VertexStepMode::Vertex,
        )];

        let mut layout = vec![
            views.layout().clone(),
            objects.layout().clone(),
            material.layout().clone(),
        ];

        if let Some(model) = model.layout().cloned() {
            layout.push(model);
        }

        let id = pipelines.queue_render_pipeline(RenderPipelineDesc {
            label: None,
            layout,
            vertex: VertexState {
                shader: *vertex_shader.as_ref(),
                entry: "main".into(),
                buffers,
            },
            fragment: Some(FragmentState {
                shader: *fragment_shader.as_ref(),
                entry: "main".into(),
                targets: vec![Some(ColorTargetState {
                    format: surface.format(),
                    blend: Some(<D::Material as Material>::Phase::mode().into()),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: D::primitive(),
            depth_stencil: Some(wgpu::DepthStencilState {
                format: surface.depth_format(),
                depth_write_enabled: match <D::Material as Material>::depth_write() {
                    DepthWrite::Auto => {
                        <D::Material as Material>::Phase::mode() == BlendMode::Opaque
                    }
                    DepthWrite::On => true,
                    DepthWrite::Off => false,
                },
                depth_compare: wgpu::CompareFunction::Always,
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            push_constants: vec![],
        });

        let pipeline = Self {
            id,
            key: MeshKey::from(D::vertex()),
            _marker: Default::default(),
        };

        commands.add(AddResource::from(pipeline));
    }
}

#[derive(ShaderType)]
pub struct BatchInfo {
    bounds: Aabb,
    index: u32,
    offset: u32,
    count: u32,
    indexed: u32,
}

#[derive(Resource)]
pub struct DrawArgs {
    pub indexed: ArrayBuffer<DrawIndexedIndirectArgs>,

    pub non_indexed: ArrayBuffer<DrawIndirectArgs>,
}

impl DrawArgs {
    pub fn new(device: &RenderDevice) -> Self {
        let indexed = ArrayBuffer::new(
            device,
            1,
            BufferUsages::STORAGE | BufferUsages::INDIRECT | BufferUsages::COPY_DST,
            Some("Indexed Draw Args".into()),
        );

        let non_indexed = ArrayBuffer::new(
            device,
            1,
            BufferUsages::STORAGE | BufferUsages::INDIRECT | BufferUsages::COPY_DST,
            Some("Non-Indexed Draw Args".into()),
        );

        Self {
            indexed,
            non_indexed,
        }
    }

    pub fn update(args: &mut Self, device: &RenderDevice) {
        args.indexed.update(device);
        args.non_indexed.update(device);
    }

    pub fn clear(args: &mut Self) {
        args.indexed.clear();
        args.non_indexed.clear();
    }
}

#[derive(Resource)]
pub struct GpuCulling {
    batches: UniformBufferArray<BatchInfo>,
    offsets: Vec<u32>,
    pipeline: PipelineId,
}

impl GpuCulling {
    const SHADER: AssetId<Shader> = AssetId::from_u128(0);

    pub fn new(device: &RenderDevice, pipelines: &mut PipelineCache) -> Self {
        let batches = UniformBufferArray::new(
            device,
            Some(BufferUsages::UNIFORM | BufferUsages::COPY_DST),
            None,
        );

        let frustum_layout = BindGroupLayoutBuilder::new()
            .with_uniform(0, ShaderStages::COMPUTE, true, None, None)
            .build(device);

        let batch_layout = BindGroupLayoutBuilder::new()
            .with_uniform(0, ShaderStages::COMPUTE, true, None, None)
            .build(device);

        let object_layout = BindGroupLayoutBuilder::new()
            .with_storage(0, ShaderStages::COMPUTE, false, true, None, None)
            .build(device);

        let draw_arg_layout = BindGroupLayoutBuilder::new()
            .with_storage(0, ShaderStages::COMPUTE, false, false, None, None)
            .with_storage(1, ShaderStages::COMPUTE, false, false, None, None)
            .with_storage(2, ShaderStages::COMPUTE, false, false, None, None)
            .with_storage(3, ShaderStages::COMPUTE, false, false, None, None)
            .build(device);

        let pipeline = pipelines.queue_compute_pipeline(crate::ComputePipelineDesc {
            label: None,
            layout: Vec::new(),
            shader: *Self::SHADER.as_ref(),
            entry: "main".into(),
        });

        Self {
            batches,
            offsets: Vec::new(),
            pipeline,
        }
    }
}

pub struct DrawCall {
    pipeline: PipelineId,
    material: MaterialId,
    mesh: AssetId<Mesh>,
    format: MeshFormat,
    offset: u64,
    function: DrawFunctionId,
}

#[derive(Resource)]
pub struct DrawCalls<P: RenderPhase>(Vec<DrawCall>, PhantomData<P>);

impl<P: RenderPhase> DrawCalls<P> {
    pub fn new() -> Self {
        Self(Vec::new(), PhantomData)
    }

    pub fn queue<D>(
        draws: Query<(&D, &MeshFilter, &GlobalTransform)>,
        draw: DrawId<D>,
        pipeline: &DrawPipeline<D>,
        meshes: &MeshAllocator,
        render_meshes: &RenderAssets<RenderMesh>,
        sub_meshes: &RenderAssets<SubMesh>,
        calls: &mut DrawCalls<P>,
        objects: &mut ObjectBuffer,
        args: &mut DrawArgs,
    ) where
        D: Drawable<View = P::View>,
    {
        #[derive(Clone, Copy, PartialEq, Eq, Hash)]
        struct BatchKey {
            material: MaterialId,
            mesh: AssetId<Mesh>,
            sub_mesh: Option<AssetId<SubMesh>>,
        }

        let mut batches = IndexMap::new();

        for (drawable, filter, transform) in draws.iter() {
            let key = BatchKey {
                material: drawable.material().into(),
                mesh: filter.mesh,
                sub_mesh: filter.sub_mesh,
            };

            batches
                .entry(key)
                .or_insert(Vec::new())
                .push(transform.matrix());
        }

        for (key, transforms) in batches {
            let Some(mesh) = meshes.vertex_slice(&key.mesh) else {
                continue;
            };

            let Some(render_mesh) = render_meshes.get(&key.mesh) else {
                continue;
            };

            let sub_mesh = match &key.sub_mesh {
                Some(id) => sub_meshes.get(id).copied(),
                None => Some(SubMesh::from(render_mesh)),
            };

            let Some(sub_mesh) = sub_mesh else {
                continue;
            };

            let instances = objects.push_batch(transforms);
            let format = render_mesh.format();

            let (index, offset, indexed) = match &format {
                MeshFormat::Indexed { .. } => {
                    let Some(index) = meshes.index_slice(&key.mesh) else {
                        continue;
                    };

                    let base_vertex = sub_mesh.start_vertex + mesh.range.start;
                    let first_index = sub_mesh.start_index + index.range.start;
                    let index = args.indexed.push(DrawIndexedIndirectArgs {
                        base_vertex: base_vertex as i32,
                        first_index,
                        index_count: sub_mesh.index_count,
                        first_instance: u32::MAX,
                        instance_count: instances.len() as u32,
                    });

                    let offset = index * std::mem::size_of::<DrawIndexedIndirectArgs>();
                    (index, offset, 1)
                }
                MeshFormat::NonIndexed => {
                    let first_vertex = sub_mesh.start_vertex + mesh.range.start;
                    let index = args.non_indexed.push(DrawIndirectArgs {
                        first_vertex,
                        vertex_count: sub_mesh.vertex_count,
                        first_instance: instances.start,
                        instance_count: instances.len() as u32,
                    });

                    let offset = index * std::mem::size_of::<DrawIndirectArgs>();
                    (index, offset, 0)
                }
            };

            let info = BatchInfo {
                bounds: *render_mesh.bounds(),
                index: index as u32,
                offset: instances.start,
                count: instances.len() as u32,
                indexed,
            };

            calls.0.push(DrawCall {
                pipeline: pipeline.id,
                material: key.material,
                mesh: key.mesh,
                format,
                offset: offset as u64,
                function: draw.0,
            });
        }
    }

    pub fn draw(
        camera: Single<
            (&ViewInstance<P::View>, &CameraAttachments),
            (With<P::View>, With<ActiveCamera>),
        >,
        calls: &DrawCalls<P>,
        meta: &SystemMeta,
        mut functions: DrawFunctions,
        mut encoder: RenderCommandEncoder,
    ) {
        let (view, attachments) = *camera;

        let Some(color) = attachments.color.as_ref() else {
            return;
        };

        let color_attachments = vec![Some(RenderPassColorAttachment {
            view: color,
            resolve_target: None,
            ops: Operations {
                load: wgpu::LoadOp::Load,
                store: wgpu::StoreOp::Store,
            },
        })];

        let pass = encoder.begin_render_pass(&RenderPassDescriptor {
            label: Some(ecs::ext::short_type_name::<P>()),
            color_attachments: &color_attachments,
            depth_stencil_attachment: Some(RenderPassDepthStencilAttachment {
                view: &attachments.depth,
                depth_ops: Some(Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        let mut state = RenderState::new(pass);

        for call in &calls.0 {
            functions.draw(&mut state, view.offset, call, meta);
        }
    }

    pub fn clear(calls: &mut DrawCalls<P>) {
        calls.0.clear();
    }
}

#[derive(Phase)]
pub struct OpaquePhase;

#[derive(Phase)]
pub struct TransparentPhase;

macro_rules! impl_draw_command {
    ($($name:ident), *) => {
        #[allow(non_snake_case)]
        impl<$($name: DrawCommand), *> DrawCommand for ($($name), *) {
            type Arg = ($($name::Arg,)*);

            fn execute(
                state: &mut RenderState,
                view: DynamicOffset,
                call: &DrawCall,
                ($($name,)*): ArgItem<Self::Arg>,
            ) -> Result<(), DrawError> {
                ($($name::execute(state, view, call, $name)?,)*);
                Ok(())
            }
        }
    };
}

variadics::variable_impl!(impl_draw_command, P, 2, 16);
