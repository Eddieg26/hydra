use super::Buffer;
use crate::{device::RenderDevice, resources::Label};
use encase::{
    DynamicUniformBuffer as EncaseDynamicUniformBuffer, ShaderType,
    UniformBuffer as EncaseUniformBuffer, internal::WriteInto,
};
use wgpu::{BindingResource, BufferUsages, DynamicOffset};

pub struct UniformBuffer<T: ShaderType + WriteInto> {
    value: T,
    data: EncaseUniformBuffer<Vec<u8>>,
    buffer: Option<Buffer>,
    label: Label,
    usages: BufferUsages,
    is_dirty: bool,
}

impl<T: ShaderType + WriteInto> UniformBuffer<T> {
    pub fn new(value: T) -> Self {
        let mut data = EncaseUniformBuffer::new(Vec::with_capacity(T::min_size().get() as usize));
        let _ = data.write(&value);

        Self {
            value,
            data,
            buffer: None,
            label: Some(std::any::type_name::<T>().into()),
            usages: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            is_dirty: true,
        }
    }

    pub fn with_label(mut self, label: Label) -> Self {
        self.label = label;
        self
    }

    pub fn with_usages(mut self, usages: BufferUsages) -> Self {
        self.usages = BufferUsages::UNIFORM | BufferUsages::COPY_DST | usages;
        self
    }

    pub fn get(&self) -> &T {
        &self.value
    }

    pub fn get_mut(&mut self) -> &mut T {
        self.is_dirty = true;
        &mut self.value
    }

    pub fn buffer(&self) -> Option<&Buffer> {
        self.buffer.as_ref()
    }

    pub fn binding(&self) -> Option<BindingResource> {
        self.buffer.as_ref().map(|b| b.as_entire_binding())
    }

    pub fn data(&self) -> &[u8] {
        self.data.as_ref().as_slice()
    }

    pub fn is_dirty(&self) -> bool {
        self.is_dirty
    }

    pub fn set(&mut self, value: T) {
        self.value = value;
        self.data.write(&self.value).unwrap();
        self.is_dirty = true;
    }

    /// Updates the buffer if it is dirty or creates a new buffer if it doesn't exist.
    /// Returns `true` only if a new buffer was created.
    pub fn update(&mut self, device: &RenderDevice) -> bool {
        if !self.is_dirty {
            return false;
        }

        match self.buffer.as_ref() {
            Some(buffer) => {
                let data = self.data.as_ref();
                device.queue.write_buffer(buffer.as_ref(), 0, &data);
                self.is_dirty = false;
                false
            }
            None => {
                let data = self.data.as_ref();
                let buffer = Buffer::with_data(device, &data, self.usages, self.label.clone());
                self.buffer = Some(buffer);
                self.is_dirty = false;
                true
            }
        }
    }
}

pub struct UniformBufferArray<T: ShaderType> {
    data: EncaseDynamicUniformBuffer<Vec<u8>>,
    buffer: Option<Buffer>,
    alignment: u32,
    label: Label,
    usages: BufferUsages,
    is_dirty: bool,
    _marker: std::marker::PhantomData<T>,
}

impl<T: ShaderType> UniformBufferArray<T> {
    pub fn new() -> Self {
        Self {
            data: EncaseDynamicUniformBuffer::new(Vec::new()),
            buffer: None,
            alignment: T::min_size().get() as u32,
            label: None,
            usages: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            is_dirty: false,
            _marker: std::marker::PhantomData,
        }
    }

    pub fn with_alignment(mut self, alignment: u32) -> Self {
        self.data = EncaseDynamicUniformBuffer::new_with_alignment(Vec::new(), alignment as u64);
        self
    }

    pub fn with_label(mut self, label: Label) -> Self {
        self.label = label;
        self
    }

    pub fn with_usages(mut self, usages: BufferUsages) -> Self {
        self.usages = BufferUsages::UNIFORM | BufferUsages::COPY_DST | usages;
        self
    }

    pub fn inner(&self) -> Option<&Buffer> {
        self.buffer.as_ref()
    }

    pub fn binding(&self) -> Option<BindingResource> {
        self.buffer.as_ref().map(|b| b.as_entire_binding())
    }

    pub fn data(&self) -> &[u8] {
        self.data.as_ref().as_slice()
    }

    pub fn is_dirty(&self) -> bool {
        self.is_dirty
    }

    pub fn alignment(&self) -> u32 {
        self.alignment
    }

    pub fn len(&self) -> usize {
        self.data.as_ref().len() / self.alignment as usize
    }

    /// Updates the buffer if it is dirty or if the size exceeds the current buffer size.
    /// Returns the size of the buffer if it was updated, or `None` if no update was needed.
    /// If the buffer is empty, it will set the buffer to `None`.
    pub fn update(&mut self, device: &RenderDevice) -> Option<u64> {
        if !self.is_dirty {
            return None;
        }

        let size = self.data.as_ref().len() as u64;
        match self.buffer.as_ref() {
            Some(buffer) if size > buffer.size() => {
                let buffer = Buffer::new(device, size, self.usages, self.label.clone());
                self.buffer = Some(buffer);
            }
            Some(buffer) => {
                let data = bytemuck::cast_slice(self.data());
                device.queue.write_buffer(buffer.as_ref(), 0, data);
                self.is_dirty = false;
                return None;
            }
            None if size == 0 => self.buffer = None,
            None => {
                let buffer = Buffer::new(device, size, self.usages, self.label.clone());
                self.buffer = Some(buffer);
            }
        }

        Some(size)
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
            .set_offset(index as wgpu::BufferAddress * self.alignment as u64);

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
