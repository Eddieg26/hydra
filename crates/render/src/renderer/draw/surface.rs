use crate::{
    BindGroup, BindGroupBuilder, BindGroupLayout, BindGroupLayoutBuilder, Buffer, RenderDevice,
    storage::StorageBufferArray, uniform::UniformBufferArray,
};
use ecs::Resource;
use encase::{ShaderType, internal::WriteInto};
use std::{num::NonZero, ops::Range};
use wgpu::{BufferUsages, ShaderStages};

pub trait ShaderData: ShaderType + WriteInto + 'static {}
impl<S: ShaderType + WriteInto + 'static> ShaderData for S {}

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
        if device.limits().max_storage_buffers_per_shader_stage > 0 {
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
                .with_uniform(
                    0,
                    ShaderStages::VERTEX | ShaderStages::COMPUTE,
                    dynamic,
                    NonZero::new(batch_size as u64),
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
                .with_storage(
                    0,
                    ShaderStages::VERTEX | ShaderStages::COMPUTE,
                    dynamic,
                    NonZero::new(batch_size as u64),
                    None,
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

        let batch_size = NonZero::new(self.batch_size as u64);
        let new_capacity = self.batch_size as usize / buffer_size.get() as usize;
        let mut bind_groups = Vec::with_capacity(new_capacity);

        for index in 0..new_capacity {
            let offset = index as u64 * self.batch_size as u64;
            let bind_group = match &self.buffer {
                GpuBufferArray::Uniform(buffer) => BindGroupBuilder::new(&self.layout)
                    .with_uniform(0, &buffer, offset, batch_size)
                    .build(device),
                GpuBufferArray::Storage(buffer) => BindGroupBuilder::new(&self.layout)
                    .with_storage(0, &buffer, offset, batch_size)
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
        let offset = ((self.0.buffer.data().len() % self.0.batch_size as usize)
            / self.0.item_size as usize)
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
