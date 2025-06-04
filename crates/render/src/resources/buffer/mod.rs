use super::{AtomicId, Label};
use crate::device::RenderDevice;
use bytemuck::NoUninit;
use std::ops::{Deref, RangeBounds};
use wgpu::{BufferSize, DynamicOffset, util::DeviceExt};

pub mod index;
pub mod storage;
pub mod uniform;
pub mod vertex;

pub use index::*;
pub use storage::*;
pub use uniform::*;
pub use vertex::*;

pub type BufferId = AtomicId<Buffer>;

pub struct Buffer {
    id: BufferId,
    label: Label,
    inner: wgpu::Buffer,
}

impl Buffer {
    pub fn new(device: &RenderDevice, size: u64, usage: wgpu::BufferUsages, label: Label) -> Self {
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: label.as_deref(),
            size,
            usage,
            mapped_at_creation: false,
        });

        Self {
            id: BufferId::new(),
            label,
            inner: buffer,
        }
    }

    pub fn with_data(
        device: &RenderDevice,
        data: &[u8],
        usage: wgpu::BufferUsages,
        label: Label,
    ) -> Self {
        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: label.as_deref(),
            contents: data,
            usage,
        });

        Self {
            id: BufferId::new(),
            label,
            inner: buffer,
        }
    }

    pub fn id(&self) -> BufferId {
        self.id
    }

    pub fn label(&self) -> &Label {
        &self.label
    }

    pub fn slice<S: RangeBounds<u64>>(&self, bounds: S) -> BufferSlice {
        BufferSlice::new(self, bounds)
    }

    pub fn size(&self) -> u64 {
        self.inner.size()
    }

    pub fn usage(&self) -> wgpu::BufferUsages {
        self.inner.usage()
    }

    pub fn as_entire_binding(&self) -> wgpu::BindingResource<'_> {
        self.inner.as_entire_binding()
    }

    pub fn as_entire_buffer_binding(&self) -> wgpu::BufferBinding<'_> {
        self.inner.as_entire_buffer_binding()
    }

    pub fn resize(&mut self, device: &RenderDevice, size: u64) {
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size,
            usage: self.inner.usage(),
            mapped_at_creation: false,
        });

        self.inner = buffer;
    }

    pub fn resize_with_data(&mut self, device: &RenderDevice, data: &[u8]) {
        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: data,
            usage: self.inner.usage(),
        });

        self.inner = buffer;
    }

    pub fn update(&mut self, device: &RenderDevice, data: &[u8]) {
        if data.len() as u64 > self.size() {
            let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: None,
                contents: data,
                usage: self.inner.usage(),
            });

            self.inner = buffer;
        } else {
            device.queue.write_buffer(&self.inner, 0, data);
        }
    }
}

impl From<wgpu::Buffer> for Buffer {
    fn from(buffer: wgpu::Buffer) -> Self {
        Self {
            id: BufferId::new(),
            label: None,
            inner: buffer,
        }
    }
}

impl AsRef<wgpu::Buffer> for Buffer {
    fn as_ref(&self) -> &wgpu::Buffer {
        &self.inner
    }
}

impl AsMut<wgpu::Buffer> for Buffer {
    fn as_mut(&mut self) -> &mut wgpu::Buffer {
        &mut self.inner
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BufferSliceId {
    pub id: BufferId,
    pub start: u64,
    pub end: u64,
}

pub struct BufferSlice<'a> {
    id: BufferId,
    start: u64,
    end: u64,
    slice: wgpu::BufferSlice<'a>,
}

impl<'a> BufferSlice<'a> {
    fn new<S: RangeBounds<u64>>(buffer: &'a Buffer, bounds: S) -> Self {
        let start = match bounds.start_bound() {
            std::ops::Bound::Included(start) => *start,
            std::ops::Bound::Excluded(start) => start + 1,
            std::ops::Bound::Unbounded => 0,
        };

        let end = match bounds.end_bound() {
            std::ops::Bound::Included(end) => *end + 1,
            std::ops::Bound::Excluded(end) => *end,
            std::ops::Bound::Unbounded => buffer.size(),
        };

        Self {
            id: buffer.id(),
            start,
            end,
            slice: buffer.inner.slice(bounds),
        }
    }

    pub fn buffer_id(&self) -> BufferId {
        self.id
    }

    pub fn id(&self) -> BufferSliceId {
        BufferSliceId {
            id: self.id,
            start: self.start,
            end: self.end,
        }
    }

    pub fn start(&self) -> u64 {
        self.start
    }

    pub fn end(&self) -> u64 {
        self.end
    }

    pub fn slice(&self) -> &wgpu::BufferSlice<'a> {
        &self.slice
    }
}

impl<'a> Deref for BufferSlice<'a> {
    type Target = wgpu::BufferSlice<'a>;

    fn deref(&self) -> &Self::Target {
        &self.slice
    }
}

pub struct ArrayBuffer<T: NoUninit> {
    values: Vec<T>,
    buffer: Buffer,
    is_dirty: bool,
}

impl<T: NoUninit> ArrayBuffer<T> {
    pub fn new(
        device: &RenderDevice,
        capacity: usize,
        usage: wgpu::BufferUsages,
        label: Label,
    ) -> Self {
        let capacity = capacity.max(1);

        let buffer = Buffer::new(
            device,
            (std::mem::size_of::<T>() * capacity) as u64,
            usage,
            label,
        );

        Self {
            values: Vec::with_capacity(capacity),
            buffer,
            is_dirty: false,
        }
    }

    pub fn values(&self) -> &[T] {
        &self.values
    }

    pub fn buffer(&self) -> &Buffer {
        &self.buffer
    }

    pub fn push(&mut self, value: T) -> DynamicOffset {
        let offset = self.values.len() as DynamicOffset;
        self.values.push(value);
        self.is_dirty = true;

        offset
    }

    pub fn insert(&mut self, index: usize, value: T) {
        self.values.insert(index, value);
        self.is_dirty = true;
    }

    pub fn set(&mut self, index: usize, value: T) {
        self.values[index] = value;
        self.is_dirty = true;
    }

    pub fn clear(&mut self) {
        self.values.clear();
        self.is_dirty = true;
    }

    pub fn remove(&mut self, index: usize) -> T {
        self.is_dirty = true;
        self.values.remove(index)
    }

    pub fn len(&self) -> usize {
        self.values.len()
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    pub fn update(&mut self, device: &RenderDevice) -> Option<BufferSize> {
        let size = (self.values.len() * std::mem::size_of::<T>()) as u64;
        if size > 0 && size > self.buffer.size() {
            let data = bytemuck::cast_slice(self.values.as_slice());
            self.buffer.resize_with_data(device, data);
            self.is_dirty = false;
            return BufferSize::new(self.buffer.size());
        } else if size > 0 && size < self.buffer.size() / 2 {
            let data = bytemuck::cast_slice(self.values.as_slice());
            self.buffer.resize_with_data(device, data);
            self.is_dirty = false;
            return BufferSize::new(self.buffer.size());
        } else if self.is_dirty {
            let data = bytemuck::cast_slice(self.values.as_slice());
            device.queue.write_buffer(self.buffer.as_ref(), 0, data);
            self.is_dirty = false;
        }

        None
    }
}
