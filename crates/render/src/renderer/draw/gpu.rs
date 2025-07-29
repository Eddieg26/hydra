use crate::{
    Aabb, ArrayBuffer, BindGroup, BindGroupBuilder, BindGroupLayout, BindGroupLayoutBuilder,
    ComputePipelineDesc, Frustum, GraphPass, MeshFormat, PipelineCache, PipelineId, RenderAssets,
    RenderDevice, RenderMesh, RenderResource, RenderSurface, ShaderData, SubMesh,
    drawable::{DrawCall, DrawMode, DrawPipeline, DrawSet, Drawable, ViewDrawSet},
    material::{Material, RenderPhase},
    storage::{StorageBuffer, StorageBufferArray},
    uniform::{UniformBuffer, UniformBufferArray},
    view::{View, ViewSet},
};
use asset::AssetId;
use ecs::{
    Entity, Query, Resource,
    unlifetime::{Read, Write},
};
use encase::ShaderType;
use math::{Mat4, Size};
use std::collections::HashMap;
use transform::GlobalTransform;
use wgpu::{
    BufferUsages, DynamicOffset, ShaderStages,
    wgt::{DrawIndexedIndirectArgs, DrawIndirectArgs},
};

#[derive(Resource)]
pub struct FrustumBuffer {
    buffer: UniformBufferArray<Frustum>,
    map: HashMap<Entity, DynamicOffset>,
}

impl FrustumBuffer {
    pub fn new(device: &RenderDevice) -> Self {
        let buffer = UniformBufferArray::new(device, None, None);
        Self {
            buffer,
            map: HashMap::new(),
        }
    }

    pub fn buffer(&self) -> &UniformBufferArray<Frustum> {
        &self.buffer
    }

    pub fn get(&self, entity: &Entity) -> Option<DynamicOffset> {
        self.map.get(entity).copied()
    }

    pub fn push(&mut self, entity: Entity, frustum: Frustum) -> u32 {
        let offset = self.buffer.push(&frustum);
        self.map.insert(entity, offset);
        offset
    }

    pub fn clear(&mut self) {
        self.map.clear();
        self.buffer.clear();
    }

    pub(crate) fn extract<V: View>(
        frustums: &mut FrustumBuffer,
        surface: &RenderSurface,
        query: Query<(Entity, &GlobalTransform, &V)>,
    ) {
        let screen_size = Size::new(surface.width() as f32, surface.height() as f32);
        for (entity, transform, view) in query.iter() {
            let clip_from_world = view.projection(screen_size) * transform.inverse().matrix();
            let view_translation = transform.translation();
            let view_backward = transform.back();
            let far = view.far();

            let frustum = Frustum::from_world_projection(
                &clip_from_world,
                &view_translation,
                &view_backward,
                far,
            );

            frustums.push(entity, frustum);
        }
    }
}

impl RenderResource for FrustumBuffer {
    type Arg = Read<RenderDevice>;

    fn extract(device: ecs::ArgItem<Self::Arg>) -> Result<Self, crate::ExtractError<()>> {
        Ok(Self::new(device))
    }
}

#[derive(Clone, Copy, ShaderType)]
pub struct RenderEntity {
    matrix: Mat4,
    batch: u32,
    instance: u32,
}

#[derive(Resource)]
pub struct RenderEntityBuffer {
    entities: StorageBufferArray<RenderEntity>,
    count: StorageBuffer<u32>,
}
impl RenderEntityBuffer {
    pub fn entities(&self) -> &StorageBufferArray<RenderEntity> {
        &self.entities
    }

    pub fn count(&self) -> &StorageBuffer<u32> {
        &self.count
    }

    pub fn push(&mut self, entity: &RenderEntity) -> (usize, DynamicOffset) {
        let offset = self.entities.push(entity);
        let index = offset / self.entities.alignment();

        (index as usize, offset)
    }
}

#[derive(Resource)]
pub struct DrawArgsBuffer {
    non_indexed: ArrayBuffer<DrawIndirectArgs>,

    indexed: ArrayBuffer<DrawIndexedIndirectArgs>,
}

impl DrawArgsBuffer {
    pub fn new(device: &RenderDevice) -> Self {
        let non_indexed = ArrayBuffer::new(
            device,
            1,
            BufferUsages::STORAGE | BufferUsages::COPY_DST,
            Some("DrawArgsBuffer".into()),
        );

        let indexed = ArrayBuffer::new(
            device,
            1,
            BufferUsages::STORAGE | BufferUsages::COPY_DST,
            Some("IndexedDrawArgsBuffer".into()),
        );

        Self {
            non_indexed,
            indexed,
        }
    }

    pub fn indexed(&self) -> &ArrayBuffer<DrawIndexedIndirectArgs> {
        &self.indexed
    }

    pub fn non_indexed(&self) -> &ArrayBuffer<DrawIndirectArgs> {
        &self.non_indexed
    }

    pub fn push(&mut self, vertex_count: u32) -> (usize, u64) {
        let index = self.non_indexed.push(DrawIndirectArgs {
            vertex_count,
            ..Default::default()
        });
        let offset = index * std::mem::size_of::<DrawIndirectArgs>();
        (index, offset as u64)
    }

    pub fn push_indexed(&mut self, index_count: u32) -> (usize, u64) {
        let index = self.indexed.push(DrawIndexedIndirectArgs {
            index_count,
            ..Default::default()
        });
        let offset = index * std::mem::size_of::<DrawIndexedIndirectArgs>();
        (index, offset as u64)
    }
}

#[derive(Resource)]
pub struct StorageDataBuffer<T: ShaderData> {
    instances: StorageBufferArray<T>,
    count: StorageBuffer<u32>,
    layout: BindGroupLayout,
    bind_group: BindGroup,
}

impl<T: ShaderData> StorageDataBuffer<T> {
    pub fn new(device: &RenderDevice) -> Self {
        let instances =
            StorageBufferArray::with_alignment(device, T::min_size().get() as u32, None, None);
        let count = StorageBuffer::new(device, 0, None, None);
        let layout = BindGroupLayoutBuilder::new().build(device);
        let bind_group = BindGroupBuilder::new(&layout).build(device);

        Self {
            instances,
            count,
            layout,
            bind_group,
        }
    }

    pub fn instances(&self) -> &StorageBufferArray<T> {
        &self.instances
    }

    pub fn count(&self) -> &StorageBuffer<u32> {
        &self.count
    }

    pub fn layout(&self) -> &BindGroupLayout {
        &self.layout
    }

    pub fn bind_group(&self) -> &BindGroup {
        &self.bind_group
    }

    pub fn push(&mut self, instance: &T) -> u32 {
        let offset = self.instances.push(instance);
        offset / self.instances.alignment()
    }

    pub fn queue<D, P>(
        views: &ViewSet<D::View>,
        drawables: &DrawSet<D>,
        pipeline: &DrawPipeline<D>,
        meshes: &RenderAssets<RenderMesh>,
        sub_meshes: &RenderAssets<SubMesh>,
        view_draw_set: &mut ViewDrawSet<D::View, <D::Material as Material>::Phase>,
        scene: &mut GpuScene,
        resources: &mut GpuDrawResources<D::Model>,
    ) where
        P: RenderPhase<View = D::View>,
        D: Drawable<Model = T>,
        D::Material: Material<Phase = P>,
    {
        let mut batches = HashMap::new();
        let mut draw_calls = Vec::with_capacity(batches.len());

        for drawable in &drawables.0 {
            batches
                .entry(drawable.key)
                .or_insert(Vec::new())
                .push(drawable);
        }

        for (key, drawables) in batches {
            let Some(mesh) = meshes.get(&key.mesh) else {
                continue;
            };

            if mesh.key() != pipeline.key() {
                continue;
            }

            let sub_mesh = match key.sub_mesh {
                Some(id) => sub_meshes.get(&id).copied(),
                None => Some(SubMesh::from(mesh)),
            };

            let Some(sub_mesh) = sub_mesh else {
                continue;
            };

            let (batch, indexed, mode) = match mesh.format() {
                MeshFormat::NonIndexed => {
                    let (batch, offset) = scene.draw_args.push(mesh.vertex_count());
                    let mode = DrawMode::Indirect { offset };

                    (batch as u32, 0, mode)
                }
                MeshFormat::Indexed { count, format } => {
                    let (batch, offset) = scene.draw_args.push_indexed(count);
                    let mode = DrawMode::IndexedIndirect { offset, format };

                    (batch as u32, 1, mode)
                }
            };

            resources.batches.push(RenderBatch {
                bounds: (*mesh.bounds()).into(),
                offset: resources.instances.len() as u32,
                count: drawables.len() as u32,
                indexed,
            });

            for drawable in drawables {
                scene.entities.push(&RenderEntity {
                    batch,
                    instance: resources.instances.push(&drawable.model()),
                    matrix: drawable.global.matrix(),
                });
            }

            draw_calls.push(DrawCall {
                material: key.material,
                mesh: key.mesh,
                sub_mesh,
                mode,
                pipeline: **pipeline,
                item: P::Item::default(),
                function: ViewDrawSet::<D::View, P>::draw::<D>,
            });
        }

        for view in views.0.keys() {
            view_draw_set
                .0
                .entry(*view)
                .or_default()
                .extend_from_slice(&draw_calls);
        }
    }
}

#[derive(Clone, Copy, ShaderType)]
pub struct RenderBatch {
    bounds: Aabb,
    offset: u32,
    count: u32,
    indexed: u32,
}

#[derive(Resource)]
pub struct RenderBatchBuffer {
    buffer: StorageBufferArray<RenderBatch>,
    batches: Vec<(DynamicOffset, u32)>,
}
impl RenderBatchBuffer {
    pub fn new(device: &RenderDevice) -> Self {
        let buffer = StorageBufferArray::new(device, None, None);
        Self {
            buffer,
            batches: Vec::new(),
        }
    }

    pub fn push(&mut self, batch: RenderBatch) {
        let offset = self.buffer.push(&batch);
        self.batches.push((offset, batch.count));
    }

    pub fn clear(&mut self) {
        self.buffer.clear();
        self.batches.clear();
    }
}

impl AsRef<StorageBufferArray<RenderBatch>> for RenderBatchBuffer {
    fn as_ref(&self) -> &StorageBufferArray<RenderBatch> {
        &self.buffer
    }
}

#[derive(Resource)]
pub struct GpuScene {
    pub draw_args: DrawArgsBuffer,
    pub entities: StorageBufferArray<RenderEntity>,
    pub entity_count: UniformBuffer<u32>,
}

impl GpuScene {
    pub fn new(device: &RenderDevice) -> Self {
        let draw_args = DrawArgsBuffer::new(device);
        let entities = StorageBufferArray::with_alignment(
            device,
            RenderEntity::min_size().get() as u32,
            None,
            None,
        );
        let entity_count = UniformBuffer::new(device, 0, None, None);

        Self {
            draw_args,
            entities,
            entity_count,
        }
    }
}

#[derive(Resource)]
pub struct GpuDrawResources<T: ShaderData> {
    pub batches: RenderBatchBuffer,
    pub instances: StorageBufferArray<T>,
    pub bind_group: BindGroup,
    pub layout: BindGroupLayout,
    pub pipeline: PipelineId,
}

impl<T: ShaderData> RenderResource for GpuDrawResources<T> {
    type Arg = (
        Read<RenderDevice>,
        Read<StorageDataBuffer<T>>,
        Read<FrustumBuffer>,
        Read<GpuScene>,
        Write<PipelineCache>,
    );

    fn extract(
        (device, data, frustums, scene, cache): ecs::ArgItem<Self::Arg>,
    ) -> Result<Self, crate::ExtractError<()>> {
        let batches = RenderBatchBuffer::new(device);
        let instances =
            StorageBufferArray::with_alignment(device, T::min_size().get() as u32, None, None);
        let layout = BindGroupLayoutBuilder::new()
            .with_uniform(0, ShaderStages::COMPUTE, true, None, None)
            .with_uniform(1, ShaderStages::COMPUTE, true, None, None)
            .with_uniform(2, ShaderStages::COMPUTE, false, None, None)
            .with_storage(3, ShaderStages::COMPUTE, false, true, None, None)
            .with_storage(4, ShaderStages::COMPUTE, false, true, None, None)
            .with_storage(5, ShaderStages::COMPUTE, false, false, None, None)
            .with_storage(6, ShaderStages::COMPUTE, false, false, None, None)
            .with_storage(7, ShaderStages::COMPUTE, false, false, None, None)
            .with_storage(8, ShaderStages::COMPUTE, false, false, None, None)
            .build(device);
        let bind_group = BindGroupBuilder::new(&layout)
            .with_uniform(0, frustums.buffer.as_ref(), 0, None)
            .with_uniform(1, batches.buffer.as_ref(), 0, None)
            .with_uniform(2, scene.entity_count.as_ref(), 0, None)
            .with_storage(3, instances.as_ref(), 0, None)
            .with_storage(4, scene.entities.as_ref(), 0, None)
            .with_storage(5, data.count.as_ref(), 0, None)
            .with_storage(6, scene.draw_args.non_indexed.as_ref(), 0, None)
            .with_storage(7, scene.draw_args.indexed.as_ref(), 0, None)
            .with_storage(8, data.instances.as_ref(), 0, None)
            .build(device);
        let pipeline = cache.queue_compute_pipeline(ComputePipelineDesc {
            label: None,
            layout: vec![layout.clone()],
            shader: AssetId::new(),
            entry: "main".into(),
        });

        Ok(Self {
            batches,
            instances,
            layout,
            bind_group,
            pipeline,
        })
    }
}

pub struct CullRenderEntities<T: ShaderData>(std::marker::PhantomData<T>);
impl<T: ShaderData> GraphPass for CullRenderEntities<T> {
    const NAME: crate::Name = "CullRenderEntities";

    fn setup(
        self,
        builder: &mut crate::PassBuilder,
    ) -> impl Fn(&mut crate::RenderContext) -> Result<(), crate::RenderGraphError> + Send + Sync + 'static
    {
        builder.name = ecs::ext::short_type_name::<Self>();
        builder.has_side_effect();
        |ctx| {
            let frustums = ctx.world().resource::<FrustumBuffer>();
            let Some(frustum) = ctx.view().and_then(|e| frustums.get(&e)) else {
                return Err(crate::RenderGraphError::MissingView);
            };

            let resources = ctx.world().resource::<GpuDrawResources<T>>();
            let mut encoder = ctx.device().create_command_encoder(&Default::default());
            let mut pass = encoder.begin_compute_pass(&Default::default());

            for (offset, size) in resources.batches.batches.iter() {
                pass.set_bind_group(0, resources.bind_group.as_ref(), &[frustum, *offset]);
                pass.dispatch_workgroups(size / 64, 0, 0);
            }

            Ok(())
        }
    }
}
