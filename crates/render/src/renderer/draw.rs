use crate::{
    Color, DepthOutput, ExtractResource, PassBuilder, RenderGraphPass, RenderOutput, SubMesh,
    array::GpuBufferArray,
    device::RenderDevice,
    renderer::{Camera, RenderContext, RenderState},
    resources::{
        BindGroup, BindGroupBuilder, BindGroupLayout, BindGroupLayoutBuilder, BlendMode, Buffer,
        DepthWrite, FragmentState, Material, MaterialBinding, MaterialLayout, Mesh, MeshLayout,
        PipelineCache, PipelineId, RenderAssets, RenderMesh, RenderPhase, RenderPipelineDesc,
        RenderResource, Shader, VertexState, uniform::UniformBufferArray,
    },
    surface::RenderSurface,
};
use asset::{AssetId, ErasedId};
use ecs::{
    ArgItem, Component, Entity, IndexMap, ReadOnly, Resource, SystemArg,
    app::Main,
    query::With,
    system::unlifetime::{Read, SCommands, SQuery, Write},
};
use encase::{ShaderType, private::WriteInto};
use std::{any::TypeId, collections::HashMap, num::NonZero, ops::Range};
use transform::GlobalTransform;
use wgpu::{BufferUsages, ColorTargetState, ShaderStages, VertexFormat, VertexStepMode};

pub trait ShaderData: Send + Sync + ShaderType + WriteInto + 'static {}
impl<S: Send + Sync + ShaderType + WriteInto + 'static> ShaderData for S {}

pub trait MeshData: ShaderData {}

pub struct BatchIndex {
    pub bind_group: usize,
    pub instances: Range<u32>,
}

pub struct MeshDataBindings<T: MeshData> {
    buffer: GpuBufferArray<T>,
    layout: BindGroupLayout,
    bind_groups: Vec<BindGroup>,
    item_size: u32,  // Size of a single item in the buffer
    batch_size: u32, // Size of a single batch in bytes
}

impl<T: MeshData> MeshDataBindings<T> {
    fn new(device: &RenderDevice, dynamic: bool) -> Self {
        let buffer = GpuBufferArray::new(device);
        let (layout, item_size, batch_size) = match &buffer {
            GpuBufferArray::Uniform(_) => {
                let item_size = if dynamic {
                    device.limits().min_uniform_buffer_offset_alignment
                } else {
                    T::min_size().get() as u32
                };

                let batch_size =
                    (device.limits().max_uniform_buffer_binding_size / item_size) * item_size;

                let layout = BindGroupLayoutBuilder::new()
                    .with_uniform(
                        0,
                        ShaderStages::VERTEX | ShaderStages::COMPUTE,
                        dynamic,
                        NonZero::new(batch_size as u64),
                        None,
                    )
                    .build(device);

                (layout, item_size, batch_size)
            }
            GpuBufferArray::Storage(_) => {
                let item_size = if dynamic {
                    device.limits().min_storage_buffer_offset_alignment
                } else {
                    T::min_size().get() as u32
                };

                let batch_size =
                    (device.limits().max_storage_buffer_binding_size / item_size) * item_size;

                let layout = BindGroupLayoutBuilder::new()
                    .with_storage(
                        0,
                        ShaderStages::VERTEX | ShaderStages::COMPUTE,
                        dynamic,
                        NonZero::new(batch_size as u64),
                        None,
                    )
                    .build(device);

                (layout, item_size, batch_size)
            }
        };

        Self {
            buffer,
            layout,
            bind_groups: Vec::new(),
            item_size,
            batch_size,
        }
    }

    fn update(&mut self, device: &RenderDevice) {
        let Some(buffer_size) = self.buffer.update(device) else {
            return;
        };

        let batch_size = NonZero::new(self.batch_size as u64);
        let new_capacity = self.batch_size as usize / buffer_size.get() as usize;
        let mut bind_groups = Vec::with_capacity(new_capacity);

        for index in 0..new_capacity {
            let offset = index as u64 * self.batch_size as u64;
            let bind_group = match &self.buffer {
                GpuBufferArray::Uniform(buffer) => BindGroupBuilder::new(&self.layout)
                    .with_uniform(0, buffer.buffer(), offset, batch_size)
                    .build(device),
                GpuBufferArray::Storage(buffer) => BindGroupBuilder::new(&self.layout)
                    .with_storage(0, buffer.buffer(), offset, batch_size)
                    .build(device),
            };
            bind_groups.push(bind_group);
        }

        self.bind_groups = bind_groups;
    }

    fn clear(&mut self) {
        self.buffer.clear();
    }
}

#[derive(Resource)]
pub struct MeshDataBuffer<T: MeshData> {
    batched: MeshDataBindings<T>,
    unbatched: MeshDataBindings<T>,
}

impl<T: MeshData> MeshDataBuffer<T> {
    pub fn new(device: &RenderDevice) -> Self {
        let batched = MeshDataBindings::new(device, false);
        let unbatched = MeshDataBindings::new(device, true);

        Self { batched, unbatched }
    }

    #[inline]
    pub fn bind_group<const BATCH: bool>(&self, index: usize) -> &BindGroup {
        if BATCH {
            &self.batched.bind_groups[index]
        } else {
            &self.unbatched.bind_groups[index]
        }
    }

    #[inline]
    pub fn bind_group_layout<const BATCH: bool>(&self) -> &BindGroupLayout {
        if BATCH {
            &self.batched.layout
        } else {
            &self.unbatched.layout
        }
    }

    pub fn batched(&self) -> &MeshDataBindings<T> {
        &self.batched
    }

    pub fn unbatched(&self) -> &MeshDataBindings<T> {
        &self.unbatched
    }

    pub fn push(&mut self, value: T) -> (usize, u32) {
        let index = self.unbatched.buffer.data().len() / self.unbatched.batch_size as usize;
        let offset = self.unbatched.buffer.push(&value);
        (index, offset)
    }

    pub fn push_batch(&mut self, values: Vec<T>) -> Vec<BatchIndex> {
        if values.is_empty() {
            return Vec::new();
        }

        let batch_count = self.batched.batch_size / self.batched.item_size;
        let offset = ((self.batched.buffer.size() % self.batched.batch_size as usize)
            / self.batched.item_size as usize)
            .min(values.len());
        let initial = self.create_batch(offset as u32, batch_count, &values[0..offset]);
        let mut batches = vec![initial];
        for values in values.chunks(batch_count as usize) {
            let batch = self.create_batch(0, batch_count, values);
            batches.push(batch);
        }

        batches
    }

    #[inline]
    fn create_batch(&mut self, offset: u32, batch_count: u32, batch: &[T]) -> BatchIndex {
        let bind_group = self.batched.buffer.size() / self.batched.batch_size as usize;
        let instances = offset..batch_count;
        for value in batch {
            self.batched.buffer.push(value);
        }

        BatchIndex {
            bind_group,
            instances,
        }
    }

    pub fn update(&mut self, device: &RenderDevice) {
        self.batched.update(device);
        self.unbatched.update(device);
    }

    pub fn clear(&mut self) {
        self.batched.clear();
        self.unbatched.clear();
    }

    pub(crate) fn update_buffer(device: &RenderDevice, buffer: &mut MeshDataBuffer<T>) {
        buffer.update(device);
    }

    pub(crate) fn clear_buffer(buffer: &mut MeshDataBuffer<T>) {
        buffer.clear();
    }
}

impl<T: MeshData> RenderResource for MeshDataBuffer<T> {
    type Arg = Read<RenderDevice>;

    fn extract(
        device: ecs::ArgItem<Self::Arg>,
    ) -> Result<Self, crate::resources::ExtractError<()>> {
        Ok(Self::new(device))
    }
}

pub trait View: Component + Clone + 'static {
    type Data: ShaderData;

    fn data(
        &self,
        screen_width: u32,
        screen_height: u32,
        transform: &GlobalTransform,
    ) -> Self::Data;
}

pub struct ExtractedView<V: View> {
    pub entity: Entity,
    pub view: V,
    pub data: V::Data,
    pub transform: GlobalTransform,
}

#[derive(Resource)]
pub struct ExtractedViews<V: View>(Vec<ExtractedView<V>>);
impl<V: View> ExtractedViews<V> {
    pub const fn new() -> Self {
        Self(Vec::new())
    }
}

impl<V: View> ExtractedViews<V> {
    pub(crate) fn extract(
        cameras: Main<SQuery<(Entity, &GlobalTransform, &V), With<Camera>>>,
        surface: &RenderSurface,
        views: &mut ExtractedViews<V>,
    ) {
        for (entity, transform, view) in cameras.iter() {
            let data = view.data(surface.width(), surface.height(), transform);
            views.0.push(ExtractedView {
                entity,
                view: view.clone(),
                data,
                transform: *transform,
            });
        }
    }

    pub(crate) fn queue(views: &mut ExtractedViews<V>, buffer: &mut ViewBuffer<V>) {
        for view in views.0.drain(..) {
            buffer.queue(view.entity, view.view, view.data, view.transform);
        }
    }
}

pub struct QueuedView<V: View> {
    pub view: V,
    pub data: V::Data,
    pub transform: GlobalTransform,
    pub dynamic_offset: u32,
}

#[derive(Resource)]
pub struct ViewBuffer<V: View> {
    views: HashMap<Entity, QueuedView<V>>,
    buffer: UniformBufferArray<V::Data>,
    bind_group: BindGroup,
    bind_group_layout: BindGroupLayout,
}

impl<V: View> ViewBuffer<V> {
    pub fn new(device: &RenderDevice) -> Self {
        let buffer = UniformBufferArray::new(device, None, Some(BufferUsages::COPY_DST));

        let bind_group_layout = BindGroupLayoutBuilder::new()
            .with_uniform(0, ShaderStages::all(), true, None, None)
            .build(device);

        let bind_group = BindGroupBuilder::new(&bind_group_layout)
            .with_uniform(0, buffer.as_ref(), 0, None)
            .build(device);

        Self {
            views: HashMap::new(),
            buffer,
            bind_group,
            bind_group_layout,
        }
    }

    pub fn get(&self, entity: &Entity) -> Option<&QueuedView<V>> {
        self.views.get(entity)
    }

    pub fn buffer(&self) -> &UniformBufferArray<V::Data> {
        &self.buffer
    }

    pub fn bind_group(&self) -> &BindGroup {
        &self.bind_group
    }

    pub fn layout(&self) -> &BindGroupLayout {
        &self.bind_group_layout
    }

    pub fn update(&mut self, device: &RenderDevice) {
        if self.buffer.update(device).is_some() {
            self.bind_group = BindGroupBuilder::new(&self.bind_group_layout)
                .with_uniform(0, self.buffer.as_ref(), 0, None)
                .build(device);
        }
    }

    pub fn queue(&mut self, entity: Entity, view: V, data: V::Data, transform: GlobalTransform) {
        let dynamic_index = self.buffer.push(&data);
        self.views.insert(
            entity,
            QueuedView {
                view,
                data,
                dynamic_offset: dynamic_index,
                transform,
            },
        );
    }

    pub fn clear(&mut self) {
        self.views.clear();
        self.buffer.clear();
    }

    pub(crate) fn update_buffer(device: &RenderDevice, buffer: &mut ViewBuffer<V>) {
        buffer.update(device);
    }

    pub(crate) fn clear_buffer(buffer: &mut ViewBuffer<V>) {
        buffer.clear();
    }
}

impl<V: View> RenderResource for ViewBuffer<V> {
    type Arg = Read<RenderDevice>;

    fn extract(
        device: ecs::ArgItem<Self::Arg>,
    ) -> Result<Self, crate::resources::ExtractError<()>> {
        Ok(Self::new(device))
    }
}

pub type DrawPhase<D> = <<D as Draw>::Material as Material>::Phase;
pub type DrawItem<D> = <<<D as Draw>::Material as Material>::Phase as RenderPhase>::Item;

pub trait Draw: Component + Clone {
    type View: View;
    type Mesh: MeshData;
    type Material: Material;

    const BATCH: bool = true;

    fn material(&self) -> AssetId<Self::Material>;

    fn mesh(&self) -> AssetId<Mesh>;

    fn sub_mesh(&self) -> Option<AssetId<SubMesh>> {
        None
    }

    fn data(&self, transform: &GlobalTransform) -> Self::Mesh;

    fn primitive_state() -> wgpu::PrimitiveState {
        wgpu::PrimitiveState::default()
    }

    fn formats() -> &'static [VertexFormat] {
        &[
            VertexFormat::Float32x3,
            VertexFormat::Float32x3,
            VertexFormat::Float32x2,
            VertexFormat::Float32x4,
        ]
    }

    fn shader() -> impl Into<AssetId<Shader>>;
}

pub trait IntoRenderItem<R: RenderPhase> {
    fn render_item(&self, view: &GlobalTransform, transform: &GlobalTransform) -> R::Item;
}

pub struct ExtractedDraw<D: Draw> {
    pub entity: Entity,
    draw: D,
    transform: GlobalTransform,
}

#[derive(Resource)]
pub struct ExtractedDraws<D: Draw>(Vec<ExtractedDraw<D>>);
impl<D: Draw> ExtractedDraws<D> {
    pub const fn new() -> Self {
        Self(Vec::new())
    }
}

impl<D: Draw> ExtractedDraws<D> {
    pub fn extract(
        query: Main<SQuery<(Entity, &GlobalTransform, &D)>>,
        draws: &mut ExtractedDraws<D>,
    ) {
        for (entity, transform, draw) in query.iter() {
            draws.0.push(ExtractedDraw {
                entity,
                draw: draw.clone(),
                transform: *transform,
            });
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BatchKey {
    pub material: ErasedId,
    pub mesh: AssetId<Mesh>,
    pub sub_mesh: Option<AssetId<SubMesh>>,
}

pub struct DrawCall<V: View, P: RenderPhase> {
    pub key: BatchKey,
    pub item: P::Item,
    pub bind_group: usize,
    pub dynamic_offset: u32,
    pub instances: Range<u32>,
    pub pipeline: PipelineId,
    pub function: DrawFunctionId<V>,
}

pub struct DrawCallRef<'a> {
    pub key: &'a BatchKey,
    pub bind_group: usize,
    pub dynamic_offset: u32,
    pub instances: &'a Range<u32>,
    pub pipeline: PipelineId,
    pub function: usize,
}

impl<'a, V: View, P: RenderPhase> From<&'a DrawCall<V, P>> for DrawCallRef<'a> {
    fn from(value: &'a DrawCall<V, P>) -> Self {
        Self {
            key: &value.key,
            bind_group: value.bind_group,
            dynamic_offset: value.dynamic_offset,
            instances: &value.instances,
            pipeline: value.pipeline,
            function: value.function.0,
        }
    }
}

#[derive(Resource)]
pub struct ViewDrawCalls<V: View, P: RenderPhase>(
    pub(crate) HashMap<Entity, Vec<DrawCall<V, P>>>,
    std::marker::PhantomData<V>,
);

impl<V: View, P: RenderPhase> ViewDrawCalls<V, P> {
    pub fn new() -> Self {
        Self(HashMap::new(), Default::default())
    }

    pub fn get(&self, entity: &Entity) -> Option<&Vec<DrawCall<V, P>>> {
        self.0.get(entity)
    }

    pub fn iter(&self) -> std::collections::hash_map::Iter<'_, Entity, Vec<DrawCall<V, P>>> {
        self.0.iter()
    }
}

impl<V: View, P: RenderPhase> ViewDrawCalls<V, P> {
    pub(crate) fn queue<D: Draw<View = V>>(
        draws: &mut ExtractedDraws<D>,
        views: &mut ViewDrawCalls<V, P>,
        mesh_buffer: &mut MeshDataBuffer<D::Mesh>,
        view_buffer: &ViewBuffer<V>,
        pipeline: &DrawPipeline<D>,
        function: DrawId<D>,
    ) where
        D: IntoRenderItem<P>,
        D::Material: Material<Phase = P>,
    {
        let function = function.0;

        for (entity, view) in view_buffer.views.iter() {
            if D::BATCH {
                let mut batches = HashMap::new();

                for (index, extracted) in draws.0.iter().enumerate() {
                    let key = BatchKey {
                        material: extracted.draw.material().into(),
                        mesh: extracted.draw.mesh(),
                        sub_mesh: extracted.draw.sub_mesh(),
                    };

                    let (draw_index, data) = batches.entry(key).or_insert((index, vec![]));
                    data.push(extracted.draw.data(&extracted.transform));

                    *draw_index = (*draw_index).min(index)
                }

                let draw_calls = batches
                    .drain()
                    .map(|(key, (draw_index, data))| {
                        let extracted = &draws.0[draw_index];
                        let item = extracted
                            .draw
                            .render_item(&view.transform, &extracted.transform);

                        let mut batch = mesh_buffer.push_batch(data);
                        batch
                            .drain(..)
                            .map(|b| DrawCall {
                                key,
                                item,
                                bind_group: b.bind_group,
                                instances: b.instances,
                                dynamic_offset: 0,
                                pipeline: pipeline.0,
                                function,
                            })
                            .collect::<Vec<_>>()
                    })
                    .flatten();

                views.0.entry(*entity).or_default().extend(draw_calls);
            } else {
                let draw_calls = draws.0.iter().map(|extracted| {
                    let (bind_group, dynamic_offset) =
                        mesh_buffer.push(extracted.draw.data(&extracted.transform));

                    let key = BatchKey {
                        material: extracted.draw.material().into(),
                        mesh: extracted.draw.mesh(),
                        sub_mesh: extracted.draw.sub_mesh(),
                    };

                    let item = extracted
                        .draw
                        .render_item(&view.transform, &extracted.transform);
                    let instances = 0..1;

                    DrawCall {
                        key,
                        item,
                        bind_group,
                        instances,
                        dynamic_offset,
                        pipeline: pipeline.0,
                        function,
                    }
                });

                views.0.entry(*entity).or_default().extend(draw_calls);
            }
        }

        draws.0.clear();
    }

    pub(crate) fn sort(view_draws: &mut ViewDrawCalls<V, P>) {
        for calls in view_draws.0.values_mut() {
            calls.sort_by_key(|call| call.item);
        }
    }

    pub(crate) fn clear_draws(view_draws: &mut ViewDrawCalls<V, P>) {
        view_draws.0.clear();
    }
}

#[derive(Resource)]
pub struct DrawPipeline<D: Draw>(PipelineId, std::marker::PhantomData<D>);
impl<D: Draw> std::ops::Deref for DrawPipeline<D> {
    type Target = PipelineId;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<D: Draw> RenderResource for DrawPipeline<D> {
    type Arg = (
        Write<PipelineCache>,
        Read<RenderSurface>,
        Option<Read<ViewBuffer<D::View>>>,
        Option<Read<MeshDataBuffer<D::Mesh>>>,
        Option<Write<MaterialLayout<D::Material>>>,
        SCommands,
    );

    fn extract(arg: ecs::ArgItem<Self::Arg>) -> Result<Self, crate::resources::ExtractError<()>> {
        let (cache, surface, views, mesh_data, layout, mut commands) = arg;

        let view_layout = match views {
            Some(views) => &views.bind_group_layout,
            None => return Err(crate::resources::ExtractError::Retry(())),
        };

        let Some(mesh_data) = mesh_data else {
            return Err(crate::resources::ExtractError::Retry(()));
        };

        let mesh_layout = if D::BATCH {
            &mesh_data.batched.layout
        } else {
            &mesh_data.unbatched.layout
        };

        let material_layout = match layout {
            Some(layout) => layout,
            None => {
                commands.add(ExtractResource::<MaterialLayout<D::Material>>::new());
                return Err(crate::resources::ExtractError::Retry(()));
            }
        };

        let vertex_shader: AssetId<Shader> = D::shader().into();
        let fragment_shader: AssetId<Shader> = D::Material::shader().into();

        assert!(
            !D::formats().is_empty(),
            "Mesh pipeline must have at least one vertex format"
        );

        let buffers = vec![MeshLayout::into_vertex_buffer_layout(
            0,
            D::formats(),
            VertexStepMode::Vertex,
        )];

        let id = cache.queue_render_pipeline(RenderPipelineDesc {
            label: None,
            layout: vec![
                view_layout.clone(),
                mesh_layout.clone(),
                material_layout.as_ref().clone(),
            ],
            vertex: VertexState {
                shader: vertex_shader.into(),
                entry: "main".into(),
                buffers,
            },
            fragment: Some(FragmentState {
                shader: fragment_shader.into(),
                entry: "main".into(),
                targets: vec![Some(ColorTargetState {
                    format: surface.format(),
                    blend: Some(<D::Material as Material>::Phase::mode().into()),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: D::primitive_state(),
            depth_stencil: Some(wgpu::DepthStencilState {
                format: surface.depth_format(),
                depth_write_enabled: matches!(
                    <D::Material as Material>::Phase::depth_write(),
                    DepthWrite::On
                ),
                depth_compare: wgpu::CompareFunction::Less,
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            push_constants: vec![],
        });

        Ok(Self(id, Default::default()))
    }
}

pub struct DrawFunctionId<V: View>(usize, std::marker::PhantomData<V>);

impl<V: View> std::ops::Deref for DrawFunctionId<V> {
    type Target = usize;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<V: View> From<usize> for DrawFunctionId<V> {
    fn from(value: usize) -> Self {
        Self(value, Default::default())
    }
}

impl<V: View> Clone for DrawFunctionId<V> {
    fn clone(&self) -> Self {
        Self(self.0.clone(), self.1.clone())
    }
}

impl<V: View> Copy for DrawFunctionId<V> {}

pub struct DrawId<D: Draw>(DrawFunctionId<D::View>);

unsafe impl<D: Draw> SystemArg for DrawId<D> {
    type Item<'world, 'state> = DrawId<D>;

    type State = DrawFunctionId<D::View>;

    fn init(world: &mut ecs::World, _: &mut ecs::WorldAccess) -> Self::State {
        let functions =
            world.get_or_insert_resource::<DrawFunctions<D::View>>(DrawFunctions::<D::View>::new);

        if D::BATCH {
            functions.register::<D, true>()
        } else {
            functions.register::<D, false>()
        }
    }

    unsafe fn get<'world, 'state>(
        state: &'state mut Self::State,
        _: ecs::world::WorldCell<'world>,
        _: &ecs::SystemMeta,
    ) -> Self::Item<'world, 'state> {
        Self(*state)
    }
}

pub type DrawFunction<V> = fn(
    &mut RenderState,
    &RenderContext,
    &RenderAssets<RenderMesh>,
    &RenderAssets<SubMesh>,
    &ViewBuffer<V>,
    &QueuedView<V>,
    &BatchKey,
    &Range<u32>,
    usize,
    u32,
);

#[derive(Resource)]
pub struct DrawFunctions<V: View>(IndexMap<TypeId, DrawFunction<V>>);

impl<V: View> DrawFunctions<V> {
    pub fn new() -> Self {
        Self(IndexMap::new())
    }

    pub fn register<D: Draw<View = V>, const BATCH: bool>(&mut self) -> DrawFunctionId<V> {
        let ty = TypeId::of::<D>();
        if let Some(id) = self.0.get_index_of(&ty).map(DrawFunctionId::<V>::from) {
            return id;
        };

        let id = DrawFunctionId(self.0.len(), Default::default());

        let f: DrawFunction<V> = |state,
                                  ctx,
                                  meshes,
                                  sub_meshes,
                                  view_buffer,
                                  view,
                                  key,
                                  instances,
                                  bind_group,
                                  dynamic_offset| {
            const VIEW_GROUP: u32 = 0;
            const MATERIAL_GROUP: u32 = 2;
            const MESH_GROUP: u32 = 1;
            const VERTEX_BUFFER_SLOT: u32 = 0;

            let Some(pipeline) = ctx
                .world()
                .try_resource::<DrawPipeline<D>>()
                .and_then(|id| ctx.get_render_pipeline(id))
            else {
                return;
            };

            let mesh_data = ctx.world().resource::<MeshDataBuffer<D::Mesh>>();

            let mesh = match meshes.get(&(key.mesh).into()) {
                Some(mesh) => mesh,
                None => return,
            };

            let sub_mesh = match key.sub_mesh.and_then(|id| sub_meshes.get(&(id.into()))) {
                Some(sub_mesh) => *sub_mesh,
                None => SubMesh::from(mesh),
            };

            let materials = ctx
                .world()
                .resource::<RenderAssets<MaterialBinding<D::Material>>>();

            let material = match materials.get(&(key.material.into())) {
                Some(material) => material,
                None => return,
            };

            let bind_group = mesh_data.bind_group::<BATCH>(bind_group);
            let vertices = sub_mesh.start_vertex..sub_mesh.start_vertex + sub_mesh.vertex_count;
            let indices = sub_mesh.start_index..sub_mesh.start_index + sub_mesh.index_count;

            state.set_pipeline(pipeline);
            state.set_bind_group(VIEW_GROUP, view_buffer.bind_group(), &[view.dynamic_offset]);
            Self::set_mesh_bind_group::<BATCH>(state, bind_group, dynamic_offset);
            state.set_bind_group(MATERIAL_GROUP, material, &[]);
            state.set_vertex_buffer(VERTEX_BUFFER_SLOT, mesh.vertex_buffer().slice(..));

            match mesh.index_buffer() {
                Some(buffer) => {
                    state.set_index_buffer(buffer.slice(..));
                    state.draw_indexed(indices, vertices.start as i32, instances.clone());
                }
                None => {
                    state.draw(vertices, instances.clone());
                }
            }
        };

        self.0.insert(ty, f);

        id
    }

    #[inline]
    fn set_mesh_bind_group<const BATCH: bool>(
        state: &mut RenderState,
        bind_group: &BindGroup,
        offset: u32,
    ) {
        if BATCH {
            state.set_bind_group(0, bind_group, &[]);
        } else {
            state.set_bind_group(0, bind_group, &[offset]);
        }
    }
}

pub trait DrawCommand {
    type Arg: ReadOnly;

    fn execute<'w>(
        state: &mut RenderState<'w>,
        view: &Entity,
        draw: DrawCallRef<'w>,
        arg: ArgItem<'w, 'w, Self::Arg>,
    ) -> Result<(), DrawError>;
}

pub struct SetPipeline;
impl DrawCommand for SetPipeline {
    type Arg = Read<PipelineCache>;

    fn execute<'w>(
        state: &mut RenderState<'w>,
        _: &Entity,
        draw: DrawCallRef<'w>,
        pipelines: ArgItem<'w, 'w, Self::Arg>,
    ) -> Result<(), DrawError> {
        let pipeline = pipelines
            .get_render_pipeline(&draw.pipeline)
            .ok_or(DrawError::Skip)?;

        Ok(state.set_pipeline(pipeline))
    }
}

pub struct SetView<V: View, const BINDING: u32>(std::marker::PhantomData<V>);

impl<V: View, const BINDING: u32> DrawCommand for SetView<V, BINDING> {
    type Arg = Read<ViewBuffer<V>>;

    fn execute<'w>(
        state: &mut RenderState<'w>,
        view: &Entity,
        _: DrawCallRef<'w>,
        view_buffer: ArgItem<'w, 'w, Self::Arg>,
    ) -> Result<(), DrawError> {
        let view = view_buffer.get(view).ok_or(DrawError::Skip)?;
        Ok(state.set_bind_group(BINDING, view_buffer.bind_group(), &[view.dynamic_offset]))
    }
}

pub struct SetMesh<M: MeshData, const BINDING: u32, const BATCH: bool>(std::marker::PhantomData<M>);
impl<M: MeshData, const BINDING: u32, const BATCH: bool> DrawCommand
    for SetMesh<M, BINDING, BATCH>
{
    type Arg = Read<MeshDataBuffer<M>>;

    fn execute<'w>(
        state: &mut RenderState<'w>,
        _: &Entity,
        draw: DrawCallRef<'w>,
        mesh_data: ArgItem<'w, 'w, Self::Arg>,
    ) -> Result<(), DrawError> {
        let bind_group = mesh_data.bind_group::<BATCH>(draw.bind_group);

        if BATCH {
            Ok(state.set_bind_group(BINDING, bind_group, &[]))
        } else {
            Ok(state.set_bind_group(BINDING, bind_group, &[draw.dynamic_offset]))
        }
    }
}

pub struct SetMaterial<M: Material, const BINDING: u32>(std::marker::PhantomData<M>);

impl<M: Material, const BINDING: u32> DrawCommand for SetMaterial<M, BINDING> {
    type Arg = Read<RenderAssets<MaterialBinding<M>>>;

    fn execute<'w>(
        state: &mut RenderState<'w>,
        _: &Entity,
        _: DrawCallRef<'w>,
        materials: ArgItem<'w, 'w, Self::Arg>,
    ) -> Result<(), DrawError> {
        let id = AssetId::<M>::new();
        let material = materials.get(&id.into()).ok_or(DrawError::Skip)?;

        Ok(state.set_bind_group(BINDING, &material, &[]))
    }
}

pub struct DrawMesh<M: MeshData, const BINDING: u32>(std::marker::PhantomData<M>);
impl<M: MeshData, const BINDING: u32> DrawCommand for DrawMesh<M, BINDING> {
    type Arg = (Read<RenderAssets<RenderMesh>>, Read<RenderAssets<SubMesh>>);

    fn execute<'w>(
        state: &mut RenderState<'w>,
        _: &Entity,
        draw: DrawCallRef<'w>,
        (meshes, sub_meshes): ArgItem<'w, 'w, Self::Arg>,
    ) -> Result<(), DrawError> {
        let DrawCallRef { key, instances, .. } = draw;

        let mesh = meshes.get(&(key.mesh).into()).ok_or(DrawError::Skip)?;
        let submesh = key
            .sub_mesh
            .and_then(|id| sub_meshes.get(&id.into()))
            .copied()
            .unwrap_or(SubMesh::from(mesh));

        let vertices = submesh.start_vertex..submesh.start_vertex + submesh.vertex_count;
        let indices = submesh.start_index..submesh.start_index + submesh.index_count;

        state.set_vertex_buffer(0, mesh.vertex_buffer().slice(..));

        match mesh.index_buffer() {
            Some(buffer) => {
                state.set_index_buffer(buffer.slice(..));
                Ok(state.draw_indexed(indices, vertices.start as i32, instances.clone()))
            }
            None => Ok(state.draw(vertices, instances.clone())),
        }
    }
}

pub enum DrawError {
    Skip,
}

pub struct RenderPhases(Vec<(fn(Entity, &RenderContext, &mut RenderState), i32)>);

impl RenderPhases {
    pub fn add_phase<V: View, P: RenderPhase>(&mut self) {
        self.0.push((
            |entity, ctx, state| {
                let functions = ctx.world().resource::<DrawFunctions<V>>();
                let views = ctx.world().resource::<ViewBuffer<V>>();
                let draw_calls = ctx.world().resource::<ViewDrawCalls<V, P>>();
                let meshes = ctx.world().resource::<RenderAssets<RenderMesh>>();
                let sub_meshes = ctx.world().resource::<RenderAssets<SubMesh>>();

                let Some(calls) = draw_calls.get(&entity) else {
                    return;
                };

                let Some(view) = views.get(&entity) else {
                    return;
                };

                for call in calls {
                    let function = functions.0[*call.function];

                    function(
                        state,
                        ctx,
                        meshes,
                        sub_meshes,
                        views,
                        view,
                        &call.key,
                        &call.instances,
                        call.bind_group,
                        call.dynamic_offset,
                    );
                }
            },
            P::QUEUE,
        ));
    }

    fn render(&self, entity: Entity, ctx: &RenderContext, mut state: RenderState) {
        for phase in &self.0 {
            phase.0(entity, &ctx, &mut state);
        }
    }
}

pub trait Renderer: Send + Sync + 'static {
    const NAME: super::Name;

    type Data: Send + Sync + 'static;

    fn setup(builder: &mut PassBuilder, phases: &mut RenderPhases) -> Self::Data;

    fn attachments<'a>(
        _ctx: &'a RenderContext<'a>,
        _data: &Self::Data,
    ) -> Vec<Option<wgpu::RenderPassColorAttachment<'a>>> {
        vec![]
    }
}

pub struct DrawPass<R: Renderer>(std::marker::PhantomData<R>);
impl<R: Renderer> DrawPass<R> {
    pub fn new() -> Self {
        Self(Default::default())
    }
}

impl<R: Renderer> RenderGraphPass for DrawPass<R> {
    const NAME: super::Name = R::NAME;

    fn setup(
        self,
        builder: &mut PassBuilder,
    ) -> impl Fn(&mut RenderContext) + Send + Sync + 'static {
        let mut phases = RenderPhases(Vec::new());
        let data = R::setup(builder, &mut phases);
        let color = builder.write::<RenderOutput>();
        let depth = builder.create::<DepthOutput>(());

        phases.0.sort_by_key(|p| p.1);

        move |ctx| {
            let Some(camera) = ctx.camera() else {
                return;
            };

            let color = ctx.get::<RenderOutput>(color);
            let depth = ctx.get::<DepthOutput>(depth);

            let mut encoder = ctx.encoder();
            let mut color_attachments = vec![Some(wgpu::RenderPassColorAttachment {
                view: color,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(camera.clear_color.unwrap_or(Color::black()).into()),
                    store: wgpu::StoreOp::Store,
                },
            })];

            color_attachments.extend(R::attachments(ctx, &data));
            let depth_stencil_attachment = wgpu::RenderPassDepthStencilAttachment {
                view: depth,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0f32),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            };

            let desc = wgpu::RenderPassDescriptor {
                label: Some(R::NAME),
                color_attachments: &color_attachments,
                depth_stencil_attachment: Some(depth_stencil_attachment),
                timestamp_writes: Default::default(),
                occlusion_query_set: Default::default(),
            };

            let state = RenderState::new(encoder.begin_render_pass(&desc));
            phases.render(camera.entity, ctx, state);

            ctx.submit(encoder.finish());
        }
    }
}
