use crate::{
    BindGroup, BindGroupBuilder, BindGroupLayout, BindGroupLayoutBuilder, Buffer, ExtractResource,
    FragmentState, Mesh, MeshFilter, MeshFormat, MeshKey, MeshLayout, PipelineCache, PipelineId,
    RenderAssets, RenderDevice, RenderMesh, RenderPipelineDesc, RenderResource, RenderState,
    RenderSurface, Shader, SubMesh, VertexState,
    allocator::MeshAllocator,
    draw::{
        BlendMode, DepthWrite, Material, MaterialId, MaterialInstance, MaterialLayout, ShaderModel,
        ShaderPhase, View, ViewBuffer, ViewInstance,
    },
};
use asset::AssetId;
use ecs::{
    Component, Entity, Query, Resource, World,
    query::With,
    unlifetime::{Read, SCommands, Write},
};
use encase::{DynamicUniformBuffer, ShaderType, internal::WriteInto};
use math::Mat4;
use std::{collections::HashMap, num::NonZero, ops::Range};
use transform::GlobalTransform;
use wgpu::{
    BufferUsages, ColorTargetState, IndexFormat, PrimitiveState, ShaderStages, VertexFormat,
    VertexStepMode,
};

pub const MAX_OBJECT_COUNT: u32 = 512;

pub trait ModelUniformData: ShaderType + WriteInto + Send + Sync + 'static {}
impl<T: ShaderType + WriteInto + Send + Sync + 'static> ModelUniformData for T {}

pub type DrawPhase<D> = <<D as Drawable>::Material as Material>::Phase;
pub type DrawModel<D> = <<<D as Drawable>::Material as Material>::Model as ShaderModel>::Base;

#[derive(Default, Clone, Copy, ShaderType)]
pub struct ModelData {
    world: Mat4,
}

#[derive(Debug)]
pub struct BatchIndex {
    pub bind_group: usize,
    pub instances: Range<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BatchKey {
    pub material: MaterialId,
    pub mesh: AssetId<Mesh>,
    pub sub_mesh: Option<AssetId<SubMesh>>,
}

#[derive(Resource)]
pub struct BatchedUniformBuffer<T: ModelUniformData> {
    buffer: Buffer,
    data: DynamicUniformBuffer<Vec<u8>>,
    batch_size: u32,
    item_size: u32,
    layout: BindGroupLayout,
    bind_groups: Vec<BindGroup>,
    is_dirty: bool,
    _marker: std::marker::PhantomData<T>,
}

impl<T: ModelUniformData> BatchedUniformBuffer<T> {
    pub fn get_batch_size(device: &RenderDevice) -> u32 {
        (device.limits().max_uniform_buffer_binding_size / std::mem::size_of::<T>() as u32)
            .min(MAX_OBJECT_COUNT)
    }

    pub fn new(device: &RenderDevice) -> Self {
        let item_size = std::mem::size_of::<T>() as u32;
        let batch_size = Self::get_batch_size(device);

        let buffer = Buffer::new(
            device,
            (batch_size * item_size) as u64,
            BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            None,
        );

        let layout = BindGroupLayoutBuilder::new()
            .with_uniform(
                0,
                ShaderStages::VERTEX | ShaderStages::COMPUTE,
                false,
                None,
                None,
            )
            .build(device);

        let bind_group = BindGroupBuilder::new(&layout)
            .with_uniform(0, &buffer, 0, NonZero::new(buffer.size()))
            .build(device);

        Self {
            buffer,
            data: DynamicUniformBuffer::new_with_alignment(Vec::new(), item_size as u64),
            batch_size,
            item_size,
            layout,
            is_dirty: false,
            bind_groups: vec![bind_group],
            _marker: Default::default(),
        }
    }

    pub fn layout(&self) -> &BindGroupLayout {
        &self.layout
    }

    pub fn bind_groups(&self) -> &[BindGroup] {
        &self.bind_groups
    }

    pub fn batch_size(&self) -> u32 {
        self.batch_size
    }

    pub fn push(&mut self, values: &[T]) -> Vec<BatchIndex> {
        if values.is_empty() {
            return Vec::new();
        }

        let len = self.data.as_ref().len() as u32 / self.item_size;
        let start = len % self.batch_size;
        let initial = ((self.batch_size - start) as usize).min(values.len());
        let mut batches = vec![self.create_batch(start, &values[..initial])];
        for values in values[initial..].chunks(self.batch_size as usize) {
            batches.push(self.create_batch(0, values));
        }

        self.is_dirty = true;

        batches
    }

    fn create_batch(&mut self, start: u32, values: &[T]) -> BatchIndex {
        let bind_group = self.data.as_ref().len() / (self.batch_size * self.item_size) as usize;
        let instances = start..start + values.len() as u32;

        for value in values {
            self.data.write(value).unwrap();
        }

        BatchIndex {
            bind_group,
            instances,
        }
    }

    pub fn update(&mut self, device: &RenderDevice) {
        if self.data.as_ref().len() as u64 > self.buffer.size() {
            let len = self.data.as_ref().len() as u32 / self.item_size;
            let padding = self.batch_size - (len % self.batch_size);
            let new_len = padding + len;
            let batch_count = new_len / self.batch_size;

            self.buffer
                .resize(device, (new_len * self.item_size) as u64);
            self.create_bind_groups(device, batch_count);
        }

        if self.is_dirty && self.data.as_ref().len() > 0 {
            self.buffer.update(device, self.data.as_ref().as_slice());
            self.is_dirty = false;
        }
    }

    fn create_bind_groups(&mut self, device: &RenderDevice, count: u32) {
        let mut bind_groups = Vec::with_capacity(count as usize);
        let batch_size = self.batch_size * self.item_size;
        for index in 0..count {
            let offset = index * batch_size;
            let size = NonZero::new(batch_size as u64);
            let bind_group = BindGroupBuilder::new(&self.layout)
                .with_uniform(0, &self.buffer, offset as u64, size)
                .build(device);
            bind_groups.push(bind_group);
        }

        self.bind_groups = bind_groups;
    }

    pub fn update_buffer(device: &RenderDevice, data: &mut Self) {
        data.update(device);
    }

    pub fn reset_buffer(data: &mut Self) {
        data.data.set_offset(0);
    }
}

impl BatchedUniformBuffer<ModelData> {
    pub fn queue<D>(
        views: Query<Entity, With<D::View>>,
        drawables: Query<(&D, &GlobalTransform, &MeshFilter)>,
        pipeline: &DrawPipeline<D>,
        meshes: &RenderAssets<RenderMesh>,
        sub_meshes: &RenderAssets<SubMesh>,
        draw_calls: &mut PhaseDrawCalls<DrawPhase<D>, DrawModel<D>>,
        instances: &mut Self,
    ) where
        D: Drawable,
    {
        for view in views.iter() {
            let mut batches = HashMap::new();

            for (draw, transform, filter) in drawables.iter() {
                let key = BatchKey {
                    material: draw.material().into(),
                    mesh: filter.mesh,
                    sub_mesh: filter.sub_mesh,
                };

                let instance = ModelData {
                    world: transform.matrix(),
                };

                batches.entry(key).or_insert(Vec::new()).push(instance);
            }

            for (key, data) in batches {
                let Some(mesh) = meshes.get(&key.mesh) else {
                    continue;
                };

                if mesh.key() != pipeline.key {
                    continue;
                }

                let sub_mesh = match key.sub_mesh {
                    Some(id) => sub_meshes.get(&id).copied(),
                    None => Some(SubMesh::from(mesh)),
                };

                let Some(sub_mesh) = sub_mesh else {
                    continue;
                };

                let mut batches = instances.push(&data);

                match mesh.format() {
                    MeshFormat::Indexed { format, .. } => draw_calls
                        .0
                        .entry(view)
                        .or_default()
                        .extend(batches.drain(..).map(|b| DrawCall {
                            material: key.material,
                            mesh: key.mesh,
                            sub_mesh,
                            mode: DrawMode::IndexedDirect {
                                bind_group: b.bind_group,
                                instances: b.instances,
                                format,
                            },
                            pipeline: pipeline.id,
                            item: <D::View as View>::Item::default(),
                            function: PhaseDrawCalls::<DrawPhase<D>, DrawModel<D>>::draw::<D>,
                        })),
                    MeshFormat::NonIndexed => {
                        draw_calls
                            .0
                            .entry(view)
                            .or_default()
                            .extend(batches.drain(..).map(|b| DrawCall {
                                material: key.material,
                                mesh: key.mesh,
                                sub_mesh,
                                mode: DrawMode::Direct {
                                    bind_group: b.bind_group,
                                    instances: b.instances,
                                },
                                pipeline: pipeline.id,
                                item: <D::View as View>::Item::default(),
                                function: PhaseDrawCalls::<DrawPhase<D>, DrawModel<D>>::draw::<D>,
                            }))
                    }
                };
            }
        }
    }
}

impl<T: ModelUniformData> RenderResource for BatchedUniformBuffer<T> {
    type Arg = Read<RenderDevice>;

    fn extract(device: ecs::ArgItem<Self::Arg>) -> Result<Self, crate::ExtractError<()>> {
        Ok(Self::new(device))
    }
}

pub trait Drawable: Component + Clone {
    type View: View;

    type Material: Material<View = Self::View>;

    fn material(&self) -> AssetId<Self::Material>;

    fn primitive() -> PrimitiveState {
        PrimitiveState::default()
    }

    fn vertex() -> &'static [VertexFormat];

    fn shader() -> impl Into<AssetId<Shader>>;
}

#[derive(Resource)]
pub struct DrawPipeline<D: Drawable> {
    pub id: PipelineId,
    pub key: MeshKey,
    _marker: std::marker::PhantomData<D>,
}

impl<D: Drawable> RenderResource for DrawPipeline<D> {
    type Arg = (
        Write<PipelineCache>,
        Read<RenderSurface>,
        Option<Read<BatchedUniformBuffer<ModelData>>>,
        Option<Read<ViewBuffer<D::View>>>,
        Option<Write<MaterialLayout<D::Material>>>,
        SCommands,
    );

    fn extract(arg: ecs::ArgItem<Self::Arg>) -> Result<Self, crate::ExtractError<()>> {
        let (cache, surface, cpu_model, views, layout, mut commands) = arg;

        let view_layout = match views {
            Some(views) => views.layout(),
            None => return Err(crate::resources::ExtractError::Retry(())),
        };

        let Some(model_layout) = cpu_model.map(|v| v.layout()) else {
            return Err(crate::resources::ExtractError::Retry(()));
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
            !D::vertex().is_empty(),
            "Mesh pipeline must have at least one vertex format"
        );

        let buffers = vec![MeshLayout::into_vertex_buffer_layout(
            0,
            D::vertex(),
            VertexStepMode::Vertex,
        )];

        let layout = vec![
            view_layout.clone(),
            model_layout.clone(),
            material_layout.as_ref().clone(),
        ];

        let id = cache.queue_render_pipeline(RenderPipelineDesc {
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
                        <<D::Material as Material>::Phase as ShaderPhase>::mode()
                            == BlendMode::Opaque
                    }
                    DepthWrite::Enabled => true,
                    DepthWrite::Disabled => false,
                },
                depth_compare: wgpu::CompareFunction::Greater,
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            push_constants: vec![],
        });

        Ok(Self {
            id,
            key: MeshKey::from(D::vertex()),
            _marker: Default::default(),
        })
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum DrawMode {
    Direct {
        bind_group: usize,
        instances: Range<u32>,
    },
    IndexedDirect {
        bind_group: usize,
        instances: Range<u32>,
        format: IndexFormat,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum DrawError {
    #[error("Draw command skipped")]
    Skip,
}

pub type Draw<V> =
    fn(&mut RenderState<'_>, ViewInstance<V>, &DrawCall<V>, &World) -> Result<(), DrawError>;

pub struct DrawCall<V: View> {
    pub material: MaterialId,
    pub mesh: AssetId<Mesh>,
    pub sub_mesh: SubMesh,
    pub mode: DrawMode,
    pub pipeline: PipelineId,
    pub item: V::Item,
    function: Draw<V>,
}

impl<V: View> DrawCall<V> {
    pub fn draw(
        &self,
        state: &mut RenderState<'_>,
        view: ViewInstance<V>,
        world: &World,
    ) -> Result<(), DrawError> {
        (self.function)(state, view, self, world)
    }
}

#[derive(Resource)]
pub struct PhaseDrawCalls<P: ShaderPhase, M: ShaderModel>(
    pub(crate) HashMap<Entity, Vec<DrawCall<P::View>>>,
    std::marker::PhantomData<M>,
);

impl<P: ShaderPhase, M: ShaderModel> PhaseDrawCalls<P, M> {
    pub fn new() -> Self {
        Self(HashMap::new(), std::marker::PhantomData)
    }

    pub(super) fn draw<D>(
        state: &mut RenderState<'_>,
        view: ViewInstance<P::View>,
        call: &DrawCall<P::View>,
        world: &World,
    ) -> Result<(), DrawError>
    where
        D: Drawable<View = P::View>,
        D::Material: Material<Phase = P>,
        <D::Material as Material>::Model: ShaderModel<Base = M>,
    {
        const VIEW: u32 = 0;
        const INSTANCES: u32 = 1;
        const MATERIAL: u32 = 2;

        let pipeline = world
            .resource::<PipelineCache>()
            .get_render_pipeline(&call.pipeline)
            .ok_or(DrawError::Skip)?;

        let views = world.resource::<ViewBuffer<D::View>>();
        let meshes = world.resource::<MeshAllocator>();
        let vertex = meshes.vertex_slice(&call.mesh).ok_or(DrawError::Skip)?;

        let materials = world.resource::<RenderAssets<MaterialInstance<D::Material>>>();
        let material = materials
            .get(&call.material.into())
            .ok_or(DrawError::Skip)?;

        state.set_pipeline(pipeline);
        state.set_vertex_buffer(0, vertex.buffer.slice(..));

        match &call.mode {
            DrawMode::Direct {
                bind_group,
                instances,
            } => {
                let models = world.resource::<BatchedUniformBuffer<ModelData>>();
                let bind_group = &models.bind_groups()[*bind_group];
                let vertices = call.sub_mesh.start_vertex + vertex.range.start
                    ..call.sub_mesh.start_vertex + call.sub_mesh.vertex_count + vertex.range.start;

                state.set_bind_group(VIEW, views.bind_group(), &[view.offset]);
                state.set_bind_group(INSTANCES, bind_group, &[]);
                state.set_bind_group(MATERIAL, &material, &[]);
                Ok(state.draw(vertices, instances.clone()))
            }
            DrawMode::IndexedDirect {
                bind_group,
                instances,
                format,
            } => {
                let index = meshes.index_slice(&call.mesh).ok_or(DrawError::Skip)?;
                let models = world.resource::<BatchedUniformBuffer<ModelData>>();
                let bind_group = &models.bind_groups()[*bind_group];
                let indices = call.sub_mesh.start_index + index.range.start
                    ..call.sub_mesh.start_index + call.sub_mesh.index_count + index.range.start;
                let base_vertex = (vertex.range.start + call.sub_mesh.start_vertex) as i32;

                state.set_index_buffer(index.buffer.slice(..), *format);
                state.set_bind_group(VIEW, views.bind_group(), &[view.offset]);
                state.set_bind_group(INSTANCES, bind_group, &[]);
                state.set_bind_group(MATERIAL, &material, &[]);
                Ok(state.draw_indexed(indices, base_vertex, instances.clone()))
            }
        }
    }
}
