use super::{Buffer, BufferId, BufferSlice, BufferSliceId};
use crate::device::RenderDevice;
use bytemuck::{Pod, Zeroable};
use std::ops::RangeBounds;
use wgpu::{BufferUsages, IndexFormat};

pub trait Index: Copy + Clone + Pod + Zeroable {
    fn format() -> wgpu::IndexFormat;
}

impl Index for u16 {
    fn format() -> wgpu::IndexFormat {
        wgpu::IndexFormat::Uint16
    }
}

impl Index for u32 {
    fn format() -> wgpu::IndexFormat {
        wgpu::IndexFormat::Uint32
    }
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct Indices {
    data: Vec<u8>,
    format: IndexFormat,
}

impl Indices {
    pub fn new<T: Index>(indices: &[T]) -> Self {
        let size = std::mem::size_of::<T>() as u64 * indices.len() as u64;
        let format = T::format();

        match format {
            wgpu::IndexFormat::Uint16 => assert!(size <= u16::MAX as u64),
            wgpu::IndexFormat::Uint32 => assert!(size <= u32::MAX as u64),
        }

        Indices {
            data: bytemuck::cast_slice(indices).to_vec(),
            format: T::format(),
        }
    }

    pub fn format(&self) -> IndexFormat {
        self.format
    }

    pub fn data(&self) -> &[u8] {
        &self.data
    }

    pub fn as_ref<T: Index>(&self) -> &[T] {
        bytemuck::cast_slice(&self.data)
    }

    pub fn len(&self) -> usize {
        match self.format {
            wgpu::IndexFormat::Uint16 => self.data.len() / std::mem::size_of::<u16>(),
            wgpu::IndexFormat::Uint32 => self.data.len() / std::mem::size_of::<u32>(),
        }
    }

    pub fn size(&self) -> u64 {
        self.data.len() as u64
    }

    pub fn push<T: Index>(&mut self, indices: &[T]) -> u64 {
        let index = self.data.len();
        self.data.extend_from_slice(bytemuck::cast_slice(indices));
        index as u64
    }

    pub fn set<T: Index>(&mut self, offset: usize, indices: &[T]) {
        let offset = offset * std::mem::size_of::<T>();
        let slice = self.data.as_mut_slice()[offset..].as_mut();
        slice.copy_from_slice(bytemuck::cast_slice(indices));
    }

    pub fn extend(&mut self, other: &Self) {
        assert!(self.format == other.format);
        self.data.extend_from_slice(&other.data);
    }

    pub fn clear(&mut self) {
        self.data.clear();
    }
}

pub struct IndexBuffer {
    inner: Buffer,
    format: IndexFormat,
    len: usize,
}

impl IndexBuffer {
    pub fn new(device: &RenderDevice, data: &Indices, usage: Option<BufferUsages>) -> Self {
        let usage = match usage {
            Some(usage) => usage | BufferUsages::INDEX,
            None => BufferUsages::INDEX,
        };

        Self {
            inner: Buffer::with_data(device, data.data(), usage, None),
            format: data.format(),
            len: data.len(),
        }
    }

    pub fn buffer(&self) -> &Buffer {
        &self.inner
    }

    pub fn format(&self) -> IndexFormat {
        self.format
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn size(&self) -> u64 {
        self.inner.size()
    }

    pub fn slice<S: RangeBounds<u64>>(&self, range: S) -> IndexSlice {
        IndexSlice {
            format: self.format,
            slice: self.inner.slice(range),
        }
    }

    pub fn update(&mut self, device: &RenderDevice, indices: &Indices) {
        let size = indices.size() as usize;
        if size > self.inner.size() as usize {
            let usage = self.inner.as_ref().usage();
            self.inner = Buffer::with_data(device, indices.data(), usage, None);
            self.len = indices.len();
            self.format = indices.format();
        } else {
            let data = indices.data();
            device.queue.write_buffer(self.inner.as_ref(), 0, data);
        }
    }
}

pub struct IndexSlice<'a> {
    format: IndexFormat,
    slice: BufferSlice<'a>,
}

impl<'a> IndexSlice<'a> {
    pub fn buffer_id(&self) -> BufferId {
        self.slice.buffer_id()
    }

    pub fn id(&self) -> BufferSliceId {
        self.slice.id()
    }

    pub fn format(&self) -> IndexFormat {
        self.format
    }

    pub fn slice(&self) -> &BufferSlice<'a> {
        &self.slice
    }
}

impl<'a> std::ops::Deref for IndexSlice<'a> {
    type Target = wgpu::BufferSlice<'a>;

    fn deref(&self) -> &Self::Target {
        &self.slice.slice()
    }
}
