use crate::{
    BindGroup, BindGroupBuilder, BindGroupLayout, BindGroupLayoutBuilder, Buffer, RenderDevice,
    RenderResource, storage::StorageBufferArray, uniform::UniformBufferArray,
};
use ecs::{Resource, system::unlifetime::Read};
use encase::{ShaderType, internal::WriteInto};
use std::{num::NonZero, ops::Range};
use wgpu::ShaderStages;

pub trait ModelData: ShaderType + WriteInto + Send + Sync + 'static {}
impl<S: ShaderType + WriteInto + Send + Sync + 'static> ModelData for S {}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum GpuBufferType {
    Uniform,
    Storage,
}

pub enum GpuBufferArray<S: ShaderType> {
    Uniform(UniformBufferArray<S>),
    Storage(StorageBufferArray<S>),
}

impl<S: ShaderType> GpuBufferArray<S> {
    pub fn get(&self) -> Option<&Buffer> {
        match self {
            GpuBufferArray::Uniform(buffer) => buffer.inner(),
            GpuBufferArray::Storage(buffer) => buffer.inner(),
        }
    }

    pub fn data(&self) -> &[u8] {
        match self {
            GpuBufferArray::Uniform(buffer) => buffer.data(),
            GpuBufferArray::Storage(buffer) => buffer.data(),
        }
    }

    pub fn update(&mut self, device: &RenderDevice) -> Option<u64> {
        match self {
            GpuBufferArray::Uniform(buffer) => buffer.update(device),
            GpuBufferArray::Storage(buffer) => buffer.update(device),
        }
    }

    pub fn ty(&self) -> GpuBufferType {
        match self {
            GpuBufferArray::Uniform(_) => GpuBufferType::Uniform,
            GpuBufferArray::Storage(_) => GpuBufferType::Storage,
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

pub struct ShaderDataBuffer<S: ModelData> {
    buffer: GpuBufferArray<S>,
    layout: BindGroupLayout,
    bind_groups: Vec<BindGroup>,
    item_size: u32,  // Size of a single item in the buffer
    batch_size: u32, // Size of a single batch in bytes
}

impl<S: ModelData> ShaderDataBuffer<S> {
    fn new(device: &RenderDevice, dynamic: bool) -> Self {
        if device.limits().max_storage_buffers_per_shader_stage == 0 {
            let item_size = if dynamic {
                device.limits().min_uniform_buffer_offset_alignment
            } else {
                S::min_size().get() as u32
            };

            let batch_size =
                (device.limits().max_uniform_buffer_binding_size / item_size) * item_size;

            let buffer = UniformBufferArray::new().with_alignment(item_size);

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

            let buffer = StorageBufferArray::new().with_alignment(item_size);

            let layout = BindGroupLayoutBuilder::new()
                .with_label(ecs::ext::short_type_name::<Self>())
                .with_storage(
                    0,
                    ShaderStages::VERTEX | ShaderStages::COMPUTE,
                    dynamic,
                    true,
                    None,
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

        let Some(buffer) = self.buffer.get() else {
            return;
        };

        let new_capacity = buffer_size as usize / self.batch_size as usize
            + (buffer_size as usize % self.batch_size as usize).min(1);
        let mut bind_groups = Vec::with_capacity(new_capacity);

        for index in 0..new_capacity {
            let offset = index as u64 * self.batch_size as u64;
            let size = NonZero::new((buffer_size - offset).min(self.batch_size as u64));
            let bind_group = match self.buffer.ty() {
                GpuBufferType::Uniform => BindGroupBuilder::new(&self.layout)
                    .with_uniform(0, &buffer, offset, size)
                    .build(device),
                GpuBufferType::Storage => BindGroupBuilder::new(&self.layout)
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
pub struct ModelDataBuffer<M: ModelData>(pub(crate) ShaderDataBuffer<M>);
impl<M: ModelData> ModelDataBuffer<M> {
    pub fn push(&mut self, value: &M) -> (usize, u32) {
        let index = self.0.buffer.data().len() / self.0.batch_size as usize;
        let offset = self.0.buffer.push(value);

        (index, offset)
    }
}

impl<M: ModelData> RenderResource for ModelDataBuffer<M> {
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
pub struct BatchedModelDataBuffer<M: ModelData>(pub(crate) ShaderDataBuffer<M>);
impl<M: ModelData> BatchedModelDataBuffer<M> {
    pub fn push<'a>(&mut self, values: &[M]) -> Vec<BatchIndex> {
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
    fn create_batch(&mut self, offset: u32, batch_count: u32, batch: &[M]) -> BatchIndex {
        let bind_group = self.0.buffer.data().len() / self.0.batch_size as usize;
        let instances = offset..offset + batch_count;
        for value in batch {
            self.0.buffer.push(value);
        }

        BatchIndex {
            bind_group,
            instances,
        }
    }
}

impl<M: ModelData> RenderResource for BatchedModelDataBuffer<M> {
    type Arg = Read<RenderDevice>;

    fn extract(device: ecs::ArgItem<Self::Arg>) -> Result<Self, crate::ExtractError<()>> {
        let buffer = ShaderDataBuffer::new(device, false);
        Ok(Self(buffer))
    }
}

pub(crate) fn update_model_data_buffers<M: ModelData>(
    device: &RenderDevice,
    model_data: &mut ModelDataBuffer<M>,
    batched_model_data: &mut BatchedModelDataBuffer<M>,
) {
    model_data.0.update(device);
    batched_model_data.0.update(device);
}

pub(crate) fn clear_model_data_buffers<M: ModelData>(
    model_data: &mut ModelDataBuffer<M>,
    batched_model_data: &mut BatchedModelDataBuffer<M>,
) {
    model_data.0.clear();
    batched_model_data.0.clear();
}
