use crate::{
    Aabb, ArrayBuffer, BindGroup, BindGroupBuilder, BindGroupLayout, BindGroupLayoutBuilder,
    MeshFormat, RenderAssets, RenderDevice, RenderMesh, RenderResource, ShaderData, SubMesh,
    drawable::{DrawCall, DrawMode, DrawPipeline, DrawSet, Drawable, ViewDrawSet},
    material::{Material, RenderPhase},
    storage::StorageBufferArray,
    view::ViewSet,
};
use ecs::{Resource, unlifetime::Read};
use encase::ShaderType;
use math::Mat4;
use std::collections::HashMap;
use wgpu::{
    BufferUsages, DynamicOffset, ShaderStages,
    wgt::{DrawIndexedIndirectArgs, DrawIndirectArgs},
};

#[derive(Clone, Copy, ShaderType)]
pub struct RenderEntity {
    batch: u32,
    instance: u32,
    bounds: Aabb,
    matrix: Mat4,
    indexed: u32,
}

#[derive(Resource)]
pub struct RenderEntityBuffer(StorageBufferArray<RenderEntity>);
impl RenderEntityBuffer {
    pub fn push(&mut self, entity: &RenderEntity) -> (usize, DynamicOffset) {
        let offset = self.0.push(entity);
        let index = offset / self.0.alignment();

        (index as usize, offset)
    }
}

impl RenderResource for RenderEntityBuffer {
    type Arg = Read<RenderDevice>;

    fn extract(device: ecs::ArgItem<Self::Arg>) -> Result<Self, crate::ExtractError<()>> {
        let buffer = StorageBufferArray::with_alignment(
            device,
            RenderEntity::min_size().get() as u32,
            None,
            None,
        );

        Ok(Self(buffer))
    }
}

#[derive(Resource)]
pub struct VisibleBuffer<T: ShaderData> {
    buffer: StorageBufferArray<T>,
    bind_group: BindGroup,
    layout: BindGroupLayout,
}

impl<T: ShaderData> VisibleBuffer<T> {
    pub fn buffer(&self) -> &StorageBufferArray<T> {
        &self.buffer
    }

    pub fn layout(&self) -> &BindGroupLayout {
        &self.layout
    }

    pub fn bind_group(&self) -> &BindGroup {
        &self.bind_group
    }
}

impl<T: ShaderData> RenderResource for VisibleBuffer<T> {
    type Arg = Read<RenderDevice>;

    fn extract(device: ecs::ArgItem<Self::Arg>) -> Result<Self, crate::ExtractError<()>> {
        let buffer =
            StorageBufferArray::with_alignment(device, T::min_size().get() as u32, None, None);
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
            .with_storage(0, buffer.as_ref(), 0, None)
            .build(device);

        Ok(Self {
            buffer,
            bind_group,
            layout,
        })
    }
}
impl<T: ShaderData> AsRef<StorageBufferArray<T>> for VisibleBuffer<T> {
    fn as_ref(&self) -> &StorageBufferArray<T> {
        &self.buffer
    }
}

#[derive(Resource)]
pub struct DrawArgsBuffer {
    non_indexed: ArrayBuffer<DrawIndirectArgs>,

    indexed: ArrayBuffer<DrawIndexedIndirectArgs>,
}

impl DrawArgsBuffer {
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

impl RenderResource for DrawArgsBuffer {
    type Arg = Read<RenderDevice>;

    fn extract(device: ecs::ArgItem<Self::Arg>) -> Result<Self, crate::ExtractError<()>> {
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

        Ok(Self {
            non_indexed,
            indexed,
        })
    }
}

#[derive(Resource)]
pub struct StorageDataBuffer<T: ShaderData>(StorageBufferArray<T>);

impl<T: ShaderData> StorageDataBuffer<T> {
    pub fn push(&mut self, instance: &T) -> u32 {
        let offset = self.0.push(instance);
        offset / self.0.alignment()
    }

    pub fn queue<D, P>(
        views: &ViewSet<D::View>,
        drawables: &DrawSet<D>,
        pipeline: &DrawPipeline<D>,
        meshes: &RenderAssets<RenderMesh>,
        sub_meshes: &RenderAssets<SubMesh>,
        view_draw_set: &mut ViewDrawSet<D::View, <D::Material as Material>::Phase>,
        draw_args: &mut DrawArgsBuffer,
        entities: &mut RenderEntityBuffer,
        instances: &mut Self,
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
                    let (batch, offset) = draw_args.push(mesh.vertex_count());
                    let mode = DrawMode::Indirect { offset };

                    (batch as u32, 0, mode)
                }
                MeshFormat::Indexed { count, format } => {
                    let (batch, offset) = draw_args.push_indexed(count);
                    let mode = DrawMode::IndexedIndirect { offset, format };

                    (batch as u32, 1, mode)
                }
            };

            for drawable in drawables {
                entities.push(&RenderEntity {
                    batch,
                    instance: instances.push(&drawable.model()),
                    bounds: (*mesh.bounds()).into(),
                    matrix: drawable.global.matrix(),
                    indexed,
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

impl<T: ShaderData> RenderResource for StorageDataBuffer<T> {
    type Arg = Read<RenderDevice>;

    fn extract(device: ecs::ArgItem<Self::Arg>) -> Result<Self, crate::ExtractError<()>> {
        let buffer =
            StorageBufferArray::with_alignment(device, T::min_size().get() as u32, None, None);
        Ok(Self(buffer))
    }
}
