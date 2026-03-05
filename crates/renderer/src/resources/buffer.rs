use crate::{
    core::RenderDevice,
    resources::{Label, id::GpuResourceId},
};
use bytemuck::Pod;
use encase::{ShaderType, internal::WriteInto};
use std::{borrow::Cow, marker::PhantomData, num::NonZero};
use wgpu::{
    BufferSize, BufferUsages,
    util::{BufferInitDescriptor, DeviceExt},
    wgt::BufferDescriptor,
};

pub type BufferId = GpuResourceId<Buffer>;

pub struct BufferDesc<'a, T = ()> {
    label: Option<&'a str>,
    data: T,
    usages: BufferUsages,
}

#[derive(Clone)]
pub struct Buffer {
    id: BufferId,
    label: Option<Label>,
    inner: wgpu::Buffer,
}

impl Buffer {
    pub fn new(device: &RenderDevice, desc: BufferDesc<u64>) -> Self {
        let id = BufferId::new();
        let label = desc.label.map(|l| Cow::Owned(l.to_owned()));
        let inner = device.create_buffer(&BufferDescriptor {
            label: desc.label,
            size: desc.data,
            usage: BufferUsages::COPY_SRC | BufferUsages::COPY_DST | desc.usages,
            mapped_at_creation: false,
        });

        Self { id, label, inner }
    }

    pub fn with_data<'a>(device: &RenderDevice, desc: BufferDesc<'a, &'a [u8]>) -> Self {
        let id = BufferId::new();
        let label = desc.label.map(|l| Cow::Owned(l.to_owned()));
        let inner = device.create_buffer_init(&BufferInitDescriptor {
            label: desc.label,
            contents: desc.data,
            usage: desc.usages,
        });

        Self { id, label, inner }
    }

    pub fn id(&self) -> BufferId {
        self.id
    }

    pub fn label(&self) -> Option<&str> {
        self.label.as_deref()
    }

    pub fn inner(&self) -> &wgpu::Buffer {
        &self.inner
    }

    pub fn resize(&mut self, device: &RenderDevice, size: u64) -> Option<BufferSize> {
        if self.inner.size() != size {
            let buffer = device.create_buffer(&BufferDescriptor {
                label: self.label.as_deref(),
                size,
                usage: self.inner.usage(),
                mapped_at_creation: false,
            });

            let mut encoder = device.create_command_encoder(&Default::default());
            encoder.copy_buffer_to_buffer(&self.inner, 0, &buffer, 0, size.min(buffer.size()));
            self.inner = buffer;

            NonZero::new(size)
        } else {
            None
        }
    }

    pub fn resize_with_data(&mut self, device: &RenderDevice, data: &[u8]) -> Option<BufferSize> {
        self.update(device, 0, data)
    }

    pub fn update(
        &mut self,
        device: &RenderDevice,
        offset: u64,
        data: &[u8],
    ) -> Option<BufferSize> {
        let size = data.len() as u64 + offset;
        if size > self.inner.size() {
            let buffer = device.create_buffer(&BufferDescriptor {
                label: self.label.as_deref(),
                size,
                usage: self.inner.usage(),
                mapped_at_creation: false,
            });

            let mut encoder = device.create_command_encoder(&Default::default());
            encoder.copy_buffer_to_buffer(&self.inner, 0, &buffer, 0, buffer.size());
            self.inner = buffer;

            device.queue().write_buffer(&self.inner, offset, data);

            NonZero::new(size)
        } else {
            device.queue().write_buffer(&self.inner, offset, data);
            None
        }
    }
}

impl std::ops::Deref for Buffer {
    type Target = wgpu::Buffer;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

pub struct UniformBuffer<T: ShaderType> {
    value: T,
    data: encase::UniformBuffer<Vec<u8>>,
    inner: Buffer,
    dirty: bool,
}

impl<T: ShaderType + WriteInto> UniformBuffer<T> {
    pub fn new(device: &RenderDevice, desc: BufferDesc<T>) -> Self {
        let mut data = encase::UniformBuffer::new(Vec::new());
        let _ = data.write(&desc.data);

        let buffer = Buffer::with_data(
            device,
            BufferDesc {
                label: desc.label,
                data: data.as_ref(),
                usages: BufferUsages::UNIFORM | desc.usages,
            },
        );

        Self {
            value: desc.data,
            data,
            inner: buffer,
            dirty: false,
        }
    }

    pub fn get(&self) -> &T {
        &self.value
    }

    pub fn inner(&self) -> &Buffer {
        &self.inner
    }

    pub fn set(&mut self, value: T) {
        self.dirty = true;
        self.value = value;
    }

    pub fn update(&mut self, device: &RenderDevice) {
        if !self.dirty {
            return;
        }

        self.dirty = false;
        let _ = self.data.write(&self.value);
        device
            .queue()
            .write_buffer(&self.inner, 0, self.data.as_ref());
    }
}

impl<T: ShaderType> std::ops::Deref for UniformBuffer<T> {
    type Target = Buffer;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

pub struct StorageBuffer<T: ShaderType> {
    value: T,
    data: encase::UniformBuffer<Vec<u8>>,
    inner: Buffer,
    dirty: bool,
}

impl<T: ShaderType + WriteInto> StorageBuffer<T> {
    pub fn new(device: &RenderDevice, desc: BufferDesc<T>) -> Self {
        let mut data = encase::UniformBuffer::new(Vec::new());
        let _ = data.write(&desc.data);

        let buffer = Buffer::with_data(
            device,
            BufferDesc {
                label: desc.label,
                data: data.as_ref(),
                usages: BufferUsages::STORAGE | desc.usages,
            },
        );

        Self {
            value: desc.data,
            data,
            inner: buffer,
            dirty: false,
        }
    }

    pub fn get(&self) -> &T {
        &self.value
    }

    pub fn inner(&self) -> &Buffer {
        &self.inner
    }

    pub fn set(&mut self, value: T) {
        self.dirty = true;
        self.value = value;
    }

    pub fn update(&mut self, device: &RenderDevice) {
        if !self.dirty {
            return;
        }

        self.dirty = false;
        let _ = self.data.write(&self.value);
        device
            .queue()
            .write_buffer(&self.inner, 0, self.data.as_ref());
    }
}

impl<T: ShaderType> std::ops::Deref for StorageBuffer<T> {
    type Target = Buffer;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

pub struct UniformArrayBuffer<T: ShaderType> {
    data: encase::DynamicUniformBuffer<Vec<u8>>,
    inner: Buffer,
    dirty: bool,
    _phantom: PhantomData<T>,
}

impl<T: ShaderType + WriteInto> UniformArrayBuffer<T> {
    /// Creates a new dynamic uniform buffer wrapper with a given alignment.
    ///
    /// - if `data` is true alignment is set to min_uniform_buffer_offset_alignment
    /// - if `data` is false alignment is set to the minimum size of T.
    pub fn new(device: &RenderDevice, desc: BufferDesc<bool>) -> Self {
        let alignment = if desc.data {
            device.limits().min_uniform_buffer_offset_alignment as u64
        } else {
            T::min_size().get().max(32)
        };

        let data = encase::DynamicUniformBuffer::new_with_alignment(Vec::new(), alignment);
        let buffer = Buffer::new(
            device,
            BufferDesc {
                label: desc.label,
                data: alignment,
                usages: BufferUsages::UNIFORM | desc.usages,
            },
        );

        Self {
            data,
            inner: buffer,
            dirty: false,
            _phantom: PhantomData,
        }
    }

    pub fn push(&mut self, value: &T) -> u64 {
        self.dirty = true;
        self.data.write(value).unwrap()
    }

    pub fn clear(&mut self) {
        self.dirty = true;
        self.data.set_offset(0);
        self.data.as_mut().clear();
    }

    pub fn update(&mut self, device: &RenderDevice) -> Option<BufferSize> {
        if !self.dirty {
            return None;
        }

        self.dirty = false;
        self.inner.update(device, 0, self.data.as_ref())
    }
}

impl<T: ShaderType> std::ops::Deref for UniformArrayBuffer<T> {
    type Target = Buffer;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

pub struct StorageArrayBuffer<T: ShaderType> {
    data: encase::DynamicStorageBuffer<Vec<u8>>,
    inner: Buffer,
    dirty: bool,
    _phantom: PhantomData<T>,
}

impl<T: ShaderType + WriteInto> StorageArrayBuffer<T> {
    /// Creates a new dynamic storage buffer wrapper with a given alignment.
    ///
    /// - if `data` is true alignment is set to min_storage_buffer_offset_alignment
    /// - if `data` is false alignment is set to the minimum size of T.
    pub fn new(device: &RenderDevice, desc: BufferDesc<bool>) -> Self {
        let alignment = if desc.data {
            device.limits().min_storage_buffer_offset_alignment as u64
        } else {
            T::min_size().get().max(32)
        };

        let data = encase::DynamicStorageBuffer::new_with_alignment(Vec::new(), alignment);
        let buffer = Buffer::new(
            device,
            BufferDesc {
                label: desc.label,
                data: alignment,
                usages: BufferUsages::STORAGE | desc.usages,
            },
        );

        Self {
            data,
            inner: buffer,
            dirty: false,
            _phantom: PhantomData,
        }
    }

    pub fn push(&mut self, value: &T) -> u64 {
        self.dirty = true;
        self.data.write(value).unwrap()
    }

    pub fn clear(&mut self) {
        self.data.set_offset(0);
        self.data.as_mut().clear();
    }

    pub fn update(&mut self, device: &RenderDevice) -> Option<BufferSize> {
        if !self.dirty {
            return None;
        }

        self.dirty = false;
        self.inner.update(device, 0, self.data.as_ref())
    }
}

impl<T: ShaderType> std::ops::Deref for StorageArrayBuffer<T> {
    type Target = Buffer;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

pub struct ArrayBuffer<T: Pod> {
    data: Vec<T>,
    inner: Buffer,
    dirty: bool,
}

impl<T: Pod> ArrayBuffer<T> {
    pub fn new(device: &RenderDevice, desc: BufferDesc<Vec<T>>) -> Self {
        let data = bytemuck::cast_slice(&desc.data);
        let buffer = Buffer::with_data(
            device,
            BufferDesc {
                label: desc.label,
                data,
                usages: desc.usages,
            },
        );

        Self {
            data: desc.data,
            inner: buffer,
            dirty: false,
        }
    }

    pub fn data(&self) -> &[T] {
        &self.data
    }

    pub fn inner(&self) -> &Buffer {
        &self.inner
    }

    pub fn push(&mut self, value: T) -> usize {
        let index = self.data.len();
        self.data.push(value);
        index
    }

    pub fn clear(&mut self) {
        self.dirty = true;
        self.data.clear();
    }

    pub fn update(&mut self, device: &RenderDevice) -> Option<BufferSize> {
        if !self.dirty {
            return None;
        }

        self.dirty = false;
        self.inner
            .update(device, 0, bytemuck::cast_slice(&self.data))
    }
}

impl<T: Pod> std::ops::Deref for ArrayBuffer<T> {
    type Target = Buffer;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}
