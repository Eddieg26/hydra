use super::Buffer;
use crate::{device::RenderDevice, resources::Label};
use encase::{
    DynamicStorageBuffer as EncaseDynamicStorageBuffer, ShaderType,
    StorageBuffer as EncaseStorageBuffer, internal::WriteInto,
};
use wgpu::{BindingResource, BufferUsages, DynamicOffset};

pub struct StorageBuffer<T: ShaderType + WriteInto> {
    value: T,
    data: EncaseStorageBuffer<Vec<u8>>,
    buffer: Buffer,
    is_dirty: bool,
}

impl<T: ShaderType + WriteInto> StorageBuffer<T> {
    pub fn new(
        device: &RenderDevice,
        value: T,
        usages: Option<BufferUsages>,
        label: Label,
    ) -> Self {
        let mut data = EncaseStorageBuffer::new(Vec::with_capacity(T::min_size().get() as usize));
        let _ = data.write(&value);

        let usages = usages.unwrap_or(BufferUsages::empty())
            | BufferUsages::STORAGE
            | BufferUsages::COPY_DST;
        let buffer = Buffer::with_data(device, data.as_ref(), usages, label);

        Self {
            value,
            data,
            buffer,
            is_dirty: false,
        }
    }

    pub fn get(&self) -> &T {
        &self.value
    }

    pub fn get_mut(&mut self) -> &mut T {
        self.is_dirty = true;
        &mut self.value
    }

    pub fn inner(&self) -> &Buffer {
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
        self.data.write(&self.value).unwrap();
        self.is_dirty = true;
    }

    /// Updates the buffer if it is dirty or creates a new buffer if it doesn't exist.
    /// Returns `true` only if a new buffer was created.
    pub fn update(&mut self, device: &RenderDevice) -> bool {
        if self.is_dirty {
            let data = self.data.as_ref();
            device.queue.write_buffer(self.buffer.as_ref(), 0, &data);
            self.is_dirty = false;
            true
        } else {
            false
        }
    }
}

impl<T: ShaderType + WriteInto> AsRef<Buffer> for StorageBuffer<T> {
    fn as_ref(&self) -> &Buffer {
        &self.buffer
    }
}

pub struct StorageBufferArray<T: ShaderType> {
    data: EncaseDynamicStorageBuffer<Vec<u8>>,
    buffer: Buffer,
    alignment: u32,
    is_dirty: bool,
    _marker: std::marker::PhantomData<T>,
}

impl<T: ShaderType> StorageBufferArray<T> {
    pub fn new(device: &RenderDevice, usages: Option<BufferUsages>, label: Label) -> Self {
        let alignment = device
            .limits()
            .min_storage_buffer_offset_alignment
            .max(T::min_size().get() as u32);

        Self::with_alignment(device, alignment, usages, label)
    }

    pub fn with_alignment(
        device: &RenderDevice,
        alignment: u32,
        usages: Option<BufferUsages>,
        label: Label,
    ) -> Self {
        let usages = usages.unwrap_or(BufferUsages::empty())
            | BufferUsages::STORAGE
            | BufferUsages::COPY_DST;

        let buffer = Buffer::new(device, alignment as u64, usages, label);

        Self {
            data: EncaseDynamicStorageBuffer::new(Vec::new()),
            buffer,
            alignment,
            is_dirty: false,
            _marker: std::marker::PhantomData,
        }
    }

    pub fn inner(&self) -> &Buffer {
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

    pub fn alignment(&self) -> u32 {
        self.alignment
    }

    pub fn len(&self) -> usize {
        self.data.as_ref().len() / self.alignment as usize
    }

    /// Updates the buffer if it is dirty or if the size exceeds the current buffer size.
    /// Returns the size of the buffer if it was updated, or `None` if no update was needed.
    pub fn update(&mut self, device: &RenderDevice) -> Option<u64> {
        if !self.is_dirty {
            return None;
        }

        self.is_dirty = false;

        let size = self.data.as_ref().len() as u64;
        if size == 0 {
            self.buffer.resize(device, self.alignment as u64);
            Some(size)
        } else if size != self.buffer.size() {
            let size = size.max(self.alignment as u64);
            self.buffer.resize_with_data(device, self.data.as_ref());
            Some(size)
        } else {
            let data = bytemuck::cast_slice(self.data());
            device.queue.write_buffer(self.buffer.as_ref(), 0, data);
            None
        }
    }
}

impl<T: ShaderType + WriteInto> StorageBufferArray<T> {
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

impl<T: ShaderType> AsRef<Buffer> for StorageBufferArray<T> {
    fn as_ref(&self) -> &Buffer {
        &self.buffer
    }
}
