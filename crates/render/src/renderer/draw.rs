use crate::{
    Color, DepthOutput, ExtractResource, PassBuilder, RenderGraphPass, RenderItem, RenderOutput,
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
    Component, Entity, IndexMap, Resource, SystemArg,
    app::Main,
    query::With,
    system::unlifetime::{Read, SCommands, SQuery, Write},
};
use encase::{ShaderType, private::WriteInto};
use std::{any::TypeId, collections::HashMap, ops::Range};
use transform::GlobalTransform;
use wgpu::{BufferUsages, ColorTargetState, ShaderStages, VertexFormat, VertexStepMode};

pub trait ShaderData: Send + Sync + ShaderType + WriteInto + 'static {}
impl<S: Send + Sync + ShaderType + WriteInto + 'static> ShaderData for S {}

pub trait MeshData: ShaderData {
    fn formats() -> &'static [VertexFormat];
}

#[derive(Resource)]
pub struct MeshDataBuffer<T: MeshData> {
    buffer: Buffer,
    data: Vec<u8>,
    offset: usize,
    _marker: std::marker::PhantomData<T>,
}

impl<T: MeshData> MeshDataBuffer<T> {
    const SIZE: usize = std::mem::size_of::<T>();

    pub fn new(device: &RenderDevice) -> Self {
        let data = vec![0u8; std::mem::size_of::<T>()];
        let buffer = Buffer::with_data(
            device,
            &data,
            BufferUsages::VERTEX | BufferUsages::COPY_DST,
            None,
        );

        Self {
            buffer,
            data,
            offset: 0,
            _marker: std::marker::PhantomData,
        }
    }

    pub fn buffer(&self) -> &Buffer {
        &self.buffer
    }

    pub fn data(&self) -> &[u8] {
        &self.data
    }

    pub fn push(&mut self, data: T) -> u32 {
        let offset = self.offset;
        let mut writer = encase::internal::Writer::new(&data, &mut self.data, offset).unwrap();
        data.write_into(&mut writer);

        self.offset += Self::SIZE;

        (offset / Self::SIZE) as u32
    }

    pub fn append(&mut self, mut data: Vec<T>) -> Range<u32> {
        let offset = self.offset;
        data.drain(..).for_each(|data| {
            self.push(data);
        });
        (offset / Self::SIZE) as u32..(self.offset / Self::SIZE) as u32
    }

    pub fn clear(&mut self) {
        self.data.clear();
        self.offset = 0;
    }

    pub fn update(&mut self, device: &RenderDevice) {
        let size = self.data.len() as u64;
        if size > self.buffer.size() {
            self.buffer.update(device, &self.data);
        } else if size > 0 && size < self.buffer.size() / 2 {
            self.buffer.resize_with_data(device, &self.data);
        } else if size > 0 {
            self.buffer.update(device, &self.data);
        } else {
            self.buffer.resize(device, Self::SIZE as u64);
        }
    }

    pub(crate) fn update_buffer(device: &RenderDevice, buffer: &mut MeshDataBuffer<T>) {
        buffer.update(&device);
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
    pub dynamic_index: u32,
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

    pub fn get(&self, entity: Entity) -> Option<&QueuedView<V>> {
        self.views.get(&entity)
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
                dynamic_index,
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

    fn data(&self, transform: &GlobalTransform) -> Self::Mesh;

    fn primitive_state() -> wgpu::PrimitiveState {
        wgpu::PrimitiveState::default()
    }

    fn formats() -> &'static [VertexFormat];

    fn shader() -> impl Into<AssetId<Shader>>;
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
}

pub struct DrawCall<V: View, P: RenderPhase> {
    pub key: BatchKey,
    pub instances: Range<u32>,
    pub item: P::Item,
    pub function: DrawFunctionId<V>,
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
        function: DrawId<D>,
    ) where
        D::Material: Material<Phase = P>,
    {
        let function = function.0;

        for (entity, view) in view_buffer.views.iter() {
            if D::BATCH && P::mode() == BlendMode::Opaque {
                let mut batches = HashMap::new();

                for (index, extracted) in draws.0.iter().enumerate() {
                    let key = BatchKey {
                        material: extracted.draw.material().into(),
                        mesh: extracted.draw.mesh(),
                    };

                    let (draw_index, data) = batches.entry(key).or_insert((index, vec![]));
                    data.push(extracted.draw.data(&extracted.transform));

                    *draw_index = (*draw_index).min(index)
                }

                let draw_calls = batches.drain().map(|(key, (draw_index, data))| {
                    let instances = mesh_buffer.append(data);
                    let item = P::Item::new(&view.transform, &draws.0[draw_index].transform);

                    DrawCall::<V, P> {
                        key,
                        item,
                        instances,
                        function,
                    }
                });

                views.0.entry(*entity).or_default().extend(draw_calls);
            } else {
                let draw_calls = draws.0.iter().map(|extracted| {
                    let offset = mesh_buffer.push(extracted.draw.data(&extracted.transform));

                    let key = BatchKey {
                        material: extracted.draw.material().into(),
                        mesh: extracted.draw.mesh(),
                    };

                    let item = P::Item::new(&view.transform, &extracted.transform);
                    let instances = offset..(offset + 1);

                    DrawCall::<V, P> {
                        key,
                        item,
                        instances,
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

    pub fn draw(
        &self,
        ctx: &RenderContext,
        state: &mut RenderState,
        entity: &Entity,
        view: &QueuedView<V>,
        views: &ViewBuffer<V>,
        meshes: &RenderAssets<RenderMesh>,
        functions: &DrawFunctions<V>,
    ) {
        let Some(calls) = self.0.get(entity) else {
            return;
        };

        for call in calls {
            let function = &functions.0[call.function.0];

            function(state, ctx, meshes, views, view, &call.key, &call.instances);
        }
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
        Option<Write<MaterialLayout<D::Material>>>,
        SCommands,
    );

    fn extract(arg: ecs::ArgItem<Self::Arg>) -> Result<Self, crate::resources::ExtractError<()>> {
        let (cache, surface, views, layout, mut commands) = arg;

        let Some(views) = views else {
            return Err(crate::resources::ExtractError::Retry(()));
        };

        let layout = match layout {
            Some(layout) => layout.clone(),
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

        assert!(
            !D::Mesh::formats().is_empty(),
            "Mesh pipeline must have at least one vertex format"
        );

        let buffers = vec![
            MeshLayout::into_vertex_buffer_layout(0, D::formats(), VertexStepMode::Vertex),
            MeshLayout::into_vertex_buffer_layout(
                D::formats().len() as u32,
                D::Mesh::formats(),
                VertexStepMode::Instance,
            ),
        ];

        let id = cache.queue_render_pipeline(RenderPipelineDesc {
            label: None,
            layout: vec![views.layout().clone(), layout.as_ref().clone()],
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

        functions.register::<D>()
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
    &ViewBuffer<V>,
    &QueuedView<V>,
    &BatchKey,
    &Range<u32>,
);

#[derive(Resource)]
pub struct DrawFunctions<V: View>(IndexMap<TypeId, DrawFunction<V>>);

impl<V: View> DrawFunctions<V> {
    pub fn new() -> Self {
        Self(IndexMap::new())
    }

    pub fn register<D: Draw<View = V>>(&mut self) -> DrawFunctionId<V> {
        let ty = TypeId::of::<D>();
        if let Some(id) = self.0.get_index_of(&ty).map(DrawFunctionId::<V>::from) {
            return id;
        };

        let id = DrawFunctionId(self.0.len(), Default::default());

        let f: DrawFunction<V> = |state, ctx, meshes, view_buffer, view, key, instances| {
            const VIEW_GROUP: u32 = 0;
            const MATERIAL_GROUP: u32 = 1;
            const VERTEX_BUFFER_SLOT: u32 = 0;
            const INSTANCE_BUFFER_SLOT: u32 = 1;

            let mesh_data = ctx.world().resource::<MeshDataBuffer<D::Mesh>>();

            let mesh = match meshes.get(&(key.mesh).into()) {
                Some(mesh) => mesh,
                None => return,
            };

            let materials = ctx
                .world()
                .resource::<RenderAssets<MaterialBinding<D::Material>>>();

            let material = match materials.get(&(key.material.into())) {
                Some(material) => material,
                None => return,
            };

            let Some(pipeline) = ctx
                .world()
                .try_resource::<DrawPipeline<D>>()
                .and_then(|id| ctx.get_render_pipeline(id))
            else {
                return;
            };

            let vertices = 0..mesh.vertex_count() as u32;
            let indices = 0..mesh.index_count() as u32;

            state.set_pipeline(pipeline);
            state.set_bind_group(VIEW_GROUP, view_buffer.bind_group(), &[view.dynamic_index]);
            state.set_bind_group(MATERIAL_GROUP, material, &[]);
            state.set_vertex_buffer(VERTEX_BUFFER_SLOT, mesh.vertex_buffer().slice(..));
            state.set_vertex_buffer(INSTANCE_BUFFER_SLOT, mesh_data.buffer().slice(..));

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
}

pub struct RenderPhases(Vec<fn(Entity, &RenderContext, &mut RenderState)>);

impl RenderPhases {
    pub fn add_phase<V: View, P: RenderPhase>(&mut self) {
        self.0.push(|entity, ctx, state| {
            let functions = ctx.world().resource::<DrawFunctions<V>>();
            let views = ctx.world().resource::<ViewBuffer<V>>();
            let draw_calls = ctx.world().resource::<ViewDrawCalls<V, P>>();
            let meshes = ctx.world().resource::<RenderAssets<RenderMesh>>();

            let Some(calls) = draw_calls.get(&entity) else {
                return;
            };

            let Some(view) = views.get(entity) else {
                return;
            };

            for call in calls {
                let function = functions.0[*call.function];

                function(state, ctx, meshes, views, view, &call.key, &call.instances);
            }
        });
    }

    fn render(&self, entity: Entity, ctx: &RenderContext, mut state: RenderState) {
        for phase in &self.0 {
            phase(entity, &ctx, &mut state);
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
