use crate::{RenderDevice, storage::StorageBufferArray, uniform::UniformBufferArray};
use encase::{
    DynamicStorageBuffer as EncaseDynamicStorageBuffer,
    DynamicUniformBuffer as EncaseDynamicUniformBuffer, ShaderType, internal::WriteInto,
};
use wgpu::{BufferUsages, DynamicOffset};

pub enum GpuBufferArray<T: ShaderType> {
    Uniform(UniformBufferArray<T>),
    Storage(StorageBufferArray<T>),
}

impl<T: ShaderType> GpuBufferArray<T> {
    pub fn new(device: &RenderDevice) -> Self {
        if device.limits().max_storage_buffers_per_shader_stage > 0 {
            let buffer = StorageBufferArray::with_alignment(
                device,
                T::min_size().get(),
                None,
                Some(BufferUsages::COPY_DST),
            );

            Self::Storage(buffer)
        } else {
            let buffer = UniformBufferArray::with_alignment(
                device,
                T::min_size().get(),
                None,
                Some(BufferUsages::COPY_DST),
            );

            Self::Uniform(buffer)
        }
    }

    pub fn data(&self) -> &[u8] {
        match self {
            GpuBufferArray::Uniform(buffer) => buffer.data(),
            GpuBufferArray::Storage(buffer) => buffer.data(),
        }
    }

    pub fn buffer(&self) -> &super::Buffer {
        match self {
            GpuBufferArray::Uniform(buffer) => buffer.buffer(),
            GpuBufferArray::Storage(buffer) => buffer.buffer(),
        }
    }

    pub fn update(&mut self, device: &RenderDevice) -> Option<std::num::NonZero<u64>> {
        match self {
            GpuBufferArray::Uniform(buffer) => buffer.update(device),
            GpuBufferArray::Storage(buffer) => buffer.update(device),
        }
    }

    pub fn is_uniform(&self) -> bool {
        match self {
            GpuBufferArray::Uniform(_) => true,
            GpuBufferArray::Storage(_) => false,
        }
    }
}

impl<T: ShaderType + WriteInto> GpuBufferArray<T> {
    pub fn writer(&mut self, alignment: u64) -> BufferWriter<'_, T> {
        match self {
            GpuBufferArray::Uniform(buffer) => {
                BufferWriter::new(buffer.data.as_mut(), alignment, true)
            }
            GpuBufferArray::Storage(buffer) => {
                BufferWriter::new(buffer.data.as_mut(), alignment, false)
            }
        }
    }

    pub fn push(&mut self, value: &T) -> u32 {
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

pub enum BufferWriterInner<'a> {
    Uniform(EncaseDynamicUniformBuffer<&'a mut Vec<u8>>),
    Storage(EncaseDynamicStorageBuffer<&'a mut Vec<u8>>),
}

pub struct BufferWriter<'a, T: ShaderType + WriteInto> {
    inner: BufferWriterInner<'a>,
    _marker: std::marker::PhantomData<T>,
}

impl<'a, T: ShaderType + WriteInto> BufferWriter<'a, T> {
    pub fn new(buffer: &'a mut Vec<u8>, alignment: u64, is_uniform: bool) -> Self {
        let inner = if is_uniform {
            BufferWriterInner::Uniform(EncaseDynamicUniformBuffer::new_with_alignment(
                buffer, alignment,
            ))
        } else {
            BufferWriterInner::Storage(EncaseDynamicStorageBuffer::new_with_alignment(
                buffer, alignment,
            ))
        };
        Self {
            inner,
            _marker: Default::default(),
        }
    }

    pub fn write(&mut self, value: &T) -> DynamicOffset {
        match &mut self.inner {
            BufferWriterInner::Uniform(inner) => inner.write(value).unwrap() as DynamicOffset,
            BufferWriterInner::Storage(inner) => inner.write(value).unwrap() as DynamicOffset,
        }
    }
}
