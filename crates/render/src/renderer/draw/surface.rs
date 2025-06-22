use crate::{
    BindGroup, BindGroupBuilder, BindGroupLayout, BindGroupLayoutBuilder, Buffer, RenderDevice,
    RenderResource, storage::StorageBufferArray, uniform::UniformBufferArray,
};
use ecs::{Resource, system::unlifetime::Read};
use encase::{ShaderType, internal::WriteInto};
use std::{num::NonZero, ops::Range};
use wgpu::{BufferUsages, ShaderStages};

pub trait ShaderData: ShaderType + WriteInto + Send + Sync + 'static {}
impl<S: ShaderType + WriteInto + Send + Sync + 'static> ShaderData for S {}

pub enum GpuBufferArray<S: ShaderType> {
    Uniform(UniformBufferArray<S>),
    Storage(StorageBufferArray<S>),
}

impl<S: ShaderType> GpuBufferArray<S> {
    pub fn data(&self) -> &[u8] {
        match self {
            GpuBufferArray::Uniform(buffer) => buffer.data(),
            GpuBufferArray::Storage(buffer) => buffer.data(),
        }
    }

    pub fn update(&mut self, device: &RenderDevice) -> Option<NonZero<u64>> {
        match self {
            GpuBufferArray::Uniform(buffer) => buffer.update(device),
            GpuBufferArray::Storage(buffer) => buffer.update(device),
        }
    }
}

impl<S: ShaderType + WriteInto> GpuBufferArray<S> {
    pub fn push(&mut self, value: &S) -> u32 {
        match self {
            GpuBufferArray::Uniform(buffer) => buffer.push(value),
            GpuBufferArray::Storage(buffer) => buffer.push(value),
        }
    }

    pub fn clear(&mut self) {
        match self {
            GpuBufferArray::Uniform(buffer) => buffer.clear(),
            GpuBufferArray::Storage(buffer) => buffer.clear(),
        }
    }
}

impl<S: ShaderType> AsRef<Buffer> for GpuBufferArray<S> {
    fn as_ref(&self) -> &Buffer {
        match self {
            GpuBufferArray::Uniform(buffer) => buffer.as_ref(),
            GpuBufferArray::Storage(buffer) => buffer.as_ref(),
        }
    }
}

pub struct ShaderDataBuffer<S: ShaderData> {
    buffer: GpuBufferArray<S>,
    layout: BindGroupLayout,
    bind_groups: Vec<BindGroup>,
    item_size: u32,  // Size of a single item in the buffer
    batch_size: u32, // Size of a single batch in bytes
}

impl<S: ShaderData> ShaderDataBuffer<S> {
    fn new(device: &RenderDevice, dynamic: bool) -> Self {
        if device.limits().max_storage_buffers_per_shader_stage == 0 {
            let item_size = if dynamic {
                device.limits().min_uniform_buffer_offset_alignment
            } else {
                S::min_size().get() as u32
            };

            let batch_size =
                (device.limits().max_uniform_buffer_binding_size / item_size) * item_size;

            let buffer = UniformBufferArray::with_alignment(
                device,
                item_size as u64,
                None,
                Some(BufferUsages::COPY_DST),
            );

            let layout = BindGroupLayoutBuilder::new()
                .with_label(ecs::ext::short_type_name::<Self>())
                .with_uniform(
                    0,
                    ShaderStages::VERTEX | ShaderStages::COMPUTE,
                    dynamic,
                    None,
                    None,
                )
                .build(device);

            Self {
                buffer: GpuBufferArray::Uniform(buffer),
                layout,
                bind_groups: Vec::new(),
                item_size,
                batch_size,
            }
        } else {
            let item_size = if dynamic {
                device.limits().min_storage_buffer_offset_alignment
            } else {
                S::min_size().get() as u32
            };

            let batch_size =
                (device.limits().max_storage_buffer_binding_size / item_size) * item_size;

            let buffer = StorageBufferArray::with_alignment(
                device,
                item_size as u64,
                None,
                Some(BufferUsages::COPY_DST),
            );

            let layout = BindGroupLayoutBuilder::new()
                .with_label(ecs::ext::short_type_name::<Self>())
                .with_storage(
                    0,
                    ShaderStages::VERTEX | ShaderStages::COMPUTE,
                    dynamic,
                    None,
                    None,
                    true,
                )
                .build(device);

            Self {
                buffer: GpuBufferArray::Storage(buffer),
                layout,
                bind_groups: Vec::new(),
                item_size,
                batch_size,
            }
        }
    }

    fn update(&mut self, device: &RenderDevice) {
        let Some(buffer_size) = self.buffer.update(device) else {
            return;
        };

        let new_capacity = buffer_size.get() as usize / self.batch_size as usize
            + (buffer_size.get() as usize % self.batch_size as usize).min(1);
        let mut bind_groups = Vec::with_capacity(new_capacity);

        for index in 0..new_capacity {
            let offset = index as u64 * self.batch_size as u64;
            let size = NonZero::new((buffer_size.get() - offset).min(self.batch_size as u64));
            let bind_group = match &self.buffer {
                GpuBufferArray::Uniform(buffer) => BindGroupBuilder::new(&self.layout)
                    .with_uniform(0, &buffer, offset, size)
                    .build(device),
                GpuBufferArray::Storage(buffer) => BindGroupBuilder::new(&self.layout)
                    .with_storage(0, &buffer, offset, size)
                    .build(device),
            };
            bind_groups.push(bind_group);
        }

        self.bind_groups = bind_groups;
    }

    pub fn layout(&self) -> &BindGroupLayout {
        &self.layout
    }

    pub fn bind_groups(&self) -> &[BindGroup] {
        &self.bind_groups
    }

    pub fn item_size(&self) -> u32 {
        self.item_size
    }

    pub fn batch_size(&self) -> u32 {
        self.batch_size
    }

    fn clear(&mut self) {
        self.buffer.clear();
    }
}

#[derive(Resource)]
pub struct MeshDataBuffer<S: ShaderData>(pub(crate) ShaderDataBuffer<S>);
impl<S: ShaderData> MeshDataBuffer<S> {
    pub fn push(&mut self, value: &S) -> (usize, u32) {
        let index = self.0.buffer.data().len() / self.0.batch_size as usize;
        let offset = self.0.buffer.push(value);

        (index, offset)
    }
}

impl<S: ShaderData> RenderResource for MeshDataBuffer<S> {
    type Arg = Read<RenderDevice>;

    fn extract(device: ecs::ArgItem<Self::Arg>) -> Result<Self, crate::ExtractError<()>> {
        let buffer = ShaderDataBuffer::new(device, true);
        Ok(Self(buffer))
    }
}

pub struct BatchIndex {
    pub bind_group: usize,
    pub instances: Range<u32>,
}

#[derive(Resource)]
pub struct BatchedMeshDataBuffer<S: ShaderData>(pub(crate) ShaderDataBuffer<S>);
impl<S: ShaderData> BatchedMeshDataBuffer<S> {
    pub fn push<'a>(&mut self, values: &[S]) -> Vec<BatchIndex> {
        if values.is_empty() {
            return Vec::new();
        }

        let batch_count = self.0.batch_size / self.0.item_size;
        let current_batch_count = ((self.0.buffer.data().len() % self.0.batch_size as usize)
            / self.0.item_size as usize) as u32;
        let remaining = (batch_count - current_batch_count).min(values.len() as u32) as usize;
        let initial =
            self.create_batch(current_batch_count, remaining as u32, &values[0..remaining]);

        let mut batches = vec![initial];
        for values in values[remaining..].chunks(batch_count as usize) {
            let batch = self.create_batch(0, batch_count, values);
            batches.push(batch);
        }

        batches
    }

    #[inline]
    fn create_batch(&mut self, offset: u32, batch_count: u32, batch: &[S]) -> BatchIndex {
        let bind_group = self.0.buffer.data().len() / self.0.batch_size as usize;
        let instances = offset..batch_count;
        for value in batch {
            self.0.buffer.push(value);
        }

        BatchIndex {
            bind_group,
            instances,
        }
    }
}

impl<S: ShaderData> RenderResource for BatchedMeshDataBuffer<S> {
    type Arg = Read<RenderDevice>;

    fn extract(device: ecs::ArgItem<Self::Arg>) -> Result<Self, crate::ExtractError<()>> {
        let buffer = ShaderDataBuffer::new(device, false);
        Ok(Self(buffer))
    }
}

pub(crate) fn update_mesh_data_buffers<S: ShaderData>(
    device: &RenderDevice,
    mesh_data: &mut MeshDataBuffer<S>,
    batched_mesh_data: &mut BatchedMeshDataBuffer<S>,
) {
    mesh_data.0.update(device);
    batched_mesh_data.0.update(device);
}

pub(crate) fn clear_mesh_data_buffers<S: ShaderData>(
    mesh_data: &mut MeshDataBuffer<S>,
    batched_mesh_data: &mut BatchedMeshDataBuffer<S>,
) {
    mesh_data.0.clear();
    batched_mesh_data.0.clear();
}
