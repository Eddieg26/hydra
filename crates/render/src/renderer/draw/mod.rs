use crate::{
    ActiveCamera, ArrayBuffer, AsBinding, BindGroup, BindGroupBuilder, BindGroupLayout,
    BindGroupLayoutBuilder, Buffer, CameraAttachments, Mesh, MeshFilter, MeshFormat, MeshKey,
    PipelineCache, PipelineId, RenderAsset, RenderAssets, RenderCommandEncoder, RenderDevice,
    RenderMesh, RenderState, Shader, SubMesh, allocator::MeshAllocator,
    uniform::UniformBufferArray,
};
use asset::{Asset, AssetId, ErasedId};
use ecs::{
    ArgItem, Component, Entity, IndexMap, Query, ReadOnly, Resource, SystemArg, SystemMeta, World,
    WorldAccess,
    query::{Single, With},
    unlifetime::Read,
    world::WorldCell,
};
use encase::ShaderType;
use math::Mat4;
use std::{any::TypeId, collections::HashMap, marker::PhantomData, ops::Range};
use transform::GlobalTransform;
use wgpu::{
    BufferUsages, DynamicOffset, Operations, PrimitiveState, RenderPassColorAttachment,
    RenderPassDepthStencilAttachment, RenderPassDescriptor, ShaderStages, VertexFormat,
    wgt::{DrawIndexedIndirectArgs, DrawIndirectArgs},
};

#[derive(Debug, Clone, Copy, ShaderType)]
pub struct RenderView {
    pub world: Mat4,
    pub view: Mat4,
    pub projection: Mat4,
}

#[derive(Debug, Clone, Copy, Component)]
pub struct ViewInstance {
    pub offset: DynamicOffset,
}

pub trait View: Component {
    fn projection(&self, width: f32, height: f32) -> Mat4;
}

#[derive(Resource)]
pub struct ViewBuffer<V: View> {
    instances: HashMap<Entity, ViewInstance>,
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
            instances: HashMap::new(),
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

    pub fn push(&mut self, entity: Entity, view: &RenderView) -> ViewInstance {
        let offset = self.buffer.push(view);
        self.instances.insert(entity, ViewInstance { offset });
        ViewInstance { offset }
    }

    pub fn update(&mut self, device: &RenderDevice) {
        if self.buffer.update(device).is_some() {
            self.bind_group = BindGroupBuilder::new(&self.layout)
                .with_uniform(0, self.buffer.as_ref(), 0, None)
                .build(device);
        }
    }
}

pub type MaterialId = ErasedId;
pub type ObjectTransform = Mat4;

#[derive(Resource)]
pub struct ObjectBuffer {
    objects: Vec<ObjectTransform>,
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

        let layout = BindGroupLayoutBuilder::new()
            .with_storage(0, ShaderStages::VERTEX, false, true, None, None)
            .build(device);

        let bind_group = BindGroupBuilder::new(&layout)
            .with_storage(0, &buffer, 0, None)
            .build(device);

        Self {
            objects: Vec::new(),
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

    pub fn update(&mut self, device: &RenderDevice) {
        let data = bytemuck::cast_slice(&self.objects);

        if self.buffer.size() < data.len() as u64 {
            self.buffer.resize_with_data(device, data);
            self.bind_group = BindGroupBuilder::new(&self.layout)
                .with_storage(0, &self.buffer, 0, None)
                .build(device);
        } else {
            self.buffer.update(device, data);
        }
    }

    pub fn clear(&mut self) {
        self.objects.clear();
    }
}

#[derive(Debug, Clone, Copy)]
pub enum BlendMode {
    Opaque,
    Transparent,
}

#[derive(Debug, Clone, Copy)]
pub enum DepthWrite {
    Auto,
    On,
    Off,
}

pub trait RenderPhase: 'static {
    type View: View;

    fn mode() -> BlendMode;
}

pub trait ShaderModel: Resource + Send + 'static {
    fn layout(&self) -> Option<&BindGroupLayout> {
        None
    }

    fn bind_group(&self) -> Option<&BindGroup> {
        None
    }
}

#[derive(Resource)]
pub struct Unlit;

pub trait Material: Asset + AsBinding + Clone {
    type Phase: RenderPhase;

    type Model: ShaderModel;

    fn depth_write() -> DepthWrite {
        DepthWrite::Auto
    }

    fn shader() -> impl Into<AssetId<Shader>>;
}

#[derive(Resource)]
pub struct MaterialLayout<M: Material>(BindGroupLayout, PhantomData<M>);
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

pub trait Drawable: Component
where
    <Self::Material as Material>::Phase: RenderPhase<View = Self::View>,
{
    type View: View;

    type Material: Material;

    fn material(&self) -> AssetId<Self::Material>;

    fn primitive() -> PrimitiveState {
        PrimitiveState::default()
    }

    fn vertex() -> &'static [VertexFormat];

    fn shader() -> impl Into<AssetId<Shader>>;
}

#[derive(Debug, Clone, Copy, thiserror::Error)]
pub enum DrawError {
    #[error("Skipped draw call")]
    Skip,
}

pub trait DrawCommand: 'static {
    type Arg: ReadOnly;

    fn execute(
        state: &mut RenderState,
        view: &ViewInstance,
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
        _: &ViewInstance,
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
        view: &ViewInstance,
        _: &DrawCall,
        views: ArgItem<Self::Arg>,
    ) -> Result<(), DrawError> {
        Ok(state.set_bind_group(GROUP, views.bind_group(), &[view.offset]))
    }
}

pub struct SetObject<const GROUP: u32>;
impl<const GROUP: u32> DrawCommand for SetObject<GROUP> {
    type Arg = Read<ObjectBuffer>;

    fn execute(
        state: &mut RenderState,
        _: &ViewInstance,
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
        _: &ViewInstance,
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
        _: &ViewInstance,
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
        _: &ViewInstance,
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
    SetShaderModel<<<D as Drawable>::Material as Material>::Model, 3>,
    ExecuteDraw,
);

pub type DrawFunction =
    Box<dyn FnMut(&mut RenderState, &ViewInstance, &DrawCall, &SystemMeta, WorldCell) + Send>;

#[derive(Resource)]
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
                      view: &ViewInstance,
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
        view: &ViewInstance,
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

    fn init(world: &mut World, access: &mut WorldAccess) -> Self::State {
        let commands = world.resource_mut::<DrawCommands>();
        // commands.add(Draw<D>);

        todo!()
    }

    unsafe fn get<'world, 'state>(
        state: &'state mut Self::State,
        _: WorldCell<'world>,
        _: &'world SystemMeta,
    ) -> Self::Item<'world, 'state> {
        *state
    }
}

pub struct DrawPipline<D: Drawable> {
    pub id: PipelineId,
    pub key: MeshKey,
    _marker: PhantomData<D>,
}

#[derive(Resource)]
pub struct DrawArgs {
    pub indexed: ArrayBuffer<DrawIndexedIndirectArgs>,

    pub non_indexed: ArrayBuffer<DrawIndirectArgs>,
}

pub struct DrawCall {
    pipeline: PipelineId,
    material: MaterialId,
    mesh: AssetId<Mesh>,
    format: MeshFormat,
    offset: u64,
    function: DrawFunctionId,
}

pub struct DrawCalls<P: RenderPhase>(Vec<DrawCall>, PhantomData<P>);

impl<P: RenderPhase> DrawCalls<P> {
    fn queue<D>(
        draws: Query<(&D, &MeshFilter, &GlobalTransform)>,
        draw: DrawId<D>,
        pipeline: &DrawPipline<D>,
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

            let offset = match &format {
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
                        first_instance: instances.start,
                        instance_count: instances.len() as u32,
                    });

                    index * std::mem::size_of::<DrawIndexedIndirectArgs>()
                }
                MeshFormat::NonIndexed => {
                    let first_vertex = sub_mesh.start_vertex + mesh.range.start;

                    let index = args.non_indexed.push(DrawIndirectArgs {
                        first_vertex,
                        vertex_count: sub_mesh.vertex_count,
                        first_instance: instances.start,
                        instance_count: instances.len() as u32,
                    });

                    index * std::mem::size_of::<DrawIndirectArgs>()
                }
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

    fn draw(
        camera: Single<(&ViewInstance, &CameraAttachments), (With<P::View>, With<ActiveCamera>)>,
        calls: &DrawCalls<P>,
        meta: &SystemMeta,
        mut functions: DrawFunctions,
        mut encoder: RenderCommandEncoder,
    ) {
        let (view, attachments) = *camera;

        let Some(color) = attachments.color.as_ref() else {
            return;
        };

        let pass = encoder.begin_render_pass(&RenderPassDescriptor {
            label: Some(ecs::ext::short_type_name::<P>()),
            color_attachments: &[Some(RenderPassColorAttachment {
                view: color,
                resolve_target: None,
                ops: Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
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
            functions.draw(&mut state, view, call, meta);
        }
    }
}
