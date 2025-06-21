use super::Buffer;
use crate::{device::RenderDevice, resources::Label};
use encase::{
    DynamicUniformBuffer as EncaseDynamicUniformBuffer, ShaderType,
    UniformBuffer as EncaseUniformBuffer,
    internal::{AlignmentValue, WriteInto},
};
use wgpu::{BindingResource, BufferSize, BufferUsages, DynamicOffset};

pub struct UniformBuffer<T: ShaderType + WriteInto> {
    value: T,
    data: EncaseUniformBuffer<Vec<u8>>,
    buffer: Buffer,
    is_dirty: bool,
}

impl<T: ShaderType + WriteInto> UniformBuffer<T> {
    pub fn new(
        device: &RenderDevice,
        value: T,
        label: Label,
        usage: Option<wgpu::BufferUsages>,
    ) -> Self {
        let mut data = EncaseUniformBuffer::new(Vec::with_capacity(T::min_size().get() as usize));
        data.write(&value).unwrap();

        let usage = match usage {
            Some(usage) => usage | BufferUsages::UNIFORM,
            None => BufferUsages::UNIFORM,
        };

        let buffer = Buffer::with_data(device, data.as_ref(), usage, label);

        Self {
            value,
            data,
            buffer,
            is_dirty: false,
        }
    }

    pub fn value(&self) -> &T {
        &self.value
    }

    pub fn buffer(&self) -> &Buffer {
        &self.buffer
    }

    pub fn binding(&self) -> BindingResource {
        self.buffer.as_entire_binding()
    }

    pub fn data(&self) -> &[u8] {
        self.data.as_ref().as_slice()
    }

    pub fn is_dirty(&self) -> bool {
        self.is_dirty
    }

    pub fn set(&mut self, value: T) {
        self.value = value;
        self.data.as_mut().clear();
        self.data.write(&self.value).unwrap();
        self.is_dirty = true;
    }

    pub fn update(&mut self, device: &RenderDevice) {
        if self.is_dirty {
            device
                .queue
                .write_buffer(self.buffer.as_ref(), 0, self.data.as_ref());
            self.is_dirty = false;
        }
    }
}

impl<T: ShaderType + WriteInto> AsRef<Buffer> for UniformBuffer<T> {
    fn as_ref(&self) -> &Buffer {
        &self.buffer
    }
}

pub struct UniformBufferArray<T: ShaderType> {
    data: EncaseDynamicUniformBuffer<Vec<u8>>,
    inner: Buffer,
    alignment: u64,
    is_dirty: bool,
    _marker: std::marker::PhantomData<T>,
}

impl<T: ShaderType> UniformBufferArray<T> {
    pub fn new(device: &RenderDevice, label: Label, usage: Option<BufferUsages>) -> Self {
        let alignment = AlignmentValue::new(T::min_size().get().next_power_of_two())
            .get()
            .max(device.limits().min_uniform_buffer_offset_alignment as u64);

        let data = EncaseDynamicUniformBuffer::new_with_alignment(Vec::new(), alignment);

        let usage = match usage {
            Some(usage) => usage | BufferUsages::UNIFORM,
            None => BufferUsages::UNIFORM,
        };

        let buffer = Buffer::new(device, alignment, usage, label);

        Self {
            data,
            inner: buffer,
            is_dirty: false,
            alignment,
            _marker: std::marker::PhantomData,
        }
    }

    pub fn inner(&self) -> &Buffer {
        &self.inner
    }

    pub fn binding(&self) -> BindingResource {
        self.inner.as_entire_binding()
    }

    pub fn data(&self) -> &[u8] {
        self.data.as_ref().as_slice()
    }

    pub fn is_dirty(&self) -> bool {
        self.is_dirty
    }

    pub fn alignment(&self) -> u64 {
        self.alignment
    }

    pub fn len(&self) -> usize {
        self.data.as_ref().len() / self.alignment as usize
    }

    /// Commits the buffer to the GPU. If the buffer is resized, the data is copied to the new buffer.
    /// If the buffer is not resized, the data is written to the buffer.
    /// Returns the new buffer size if the buffer was resized.
    pub fn update(&mut self, device: &RenderDevice) -> Option<BufferSize> {
        let size = self.data.as_ref().len() as u64;
        if size > 0 && size > self.inner.size() {
            let data = self.data.as_ref().as_slice();
            self.inner.resize_with_data(device, data);
            self.is_dirty = false;
            return BufferSize::new(self.inner.size());
        } else if size > 0 && size < self.inner.size() / 2 {
            let data = self.data.as_ref().as_slice();
            self.inner.resize_with_data(device, data);
            self.is_dirty = false;
            return BufferSize::new(self.inner.size());
        } else if self.is_dirty {
            let data = self.data.as_ref().as_slice();
            device.queue.write_buffer(self.inner.as_ref(), 0, data);
            self.is_dirty = false;
        }

        None
    }
}

impl<T: ShaderType + WriteInto> UniformBufferArray<T> {
    pub fn push(&mut self, value: &T) -> DynamicOffset {
        self.is_dirty = true;
        self.data.write(value).unwrap() as DynamicOffset
    }

    pub fn set(&mut self, index: usize, values: impl IntoIterator<Item = T>) -> Vec<DynamicOffset> {
        self.is_dirty = true;
        self.data
            .set_offset(index as wgpu::BufferAddress * self.alignment);

        let offsets = values
            .into_iter()
            .map(|value| self.data.write(&value).unwrap() as DynamicOffset)
            .collect();

        self.data.set_offset(self.data.as_ref().len() as u64);

        offsets
    }

    pub fn clear(&mut self) {
        self.data.as_mut().clear();
        self.data.set_offset(0);
        self.is_dirty = true;
    }
}

impl<T: ShaderType> AsRef<Buffer> for UniformBufferArray<T> {
    fn as_ref(&self) -> &Buffer {
        &self.inner
    }
}
