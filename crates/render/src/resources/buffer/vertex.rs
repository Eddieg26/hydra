use super::{Buffer, BufferSlice};
use crate::device::RenderDevice;
use bytemuck::{Pod, Zeroable};
use std::ops::RangeBounds;
use wgpu::{BufferAddress, BufferUsages};

pub struct VertexBuffer {
    buffer: Buffer,
    len: usize,
}

impl VertexBuffer {
    pub fn new<T: Pod + Zeroable>(
        device: &RenderDevice,
        vertices: &[T],
        usage: Option<BufferUsages>,
    ) -> Self {
        let usage = match usage {
            Some(usage) => usage | BufferUsages::VERTEX,
            None => BufferUsages::VERTEX,
        };

        Self {
            buffer: Buffer::with_data(device, bytemuck::cast_slice(vertices), usage, None),
            len: vertices.len(),
        }
    }

    pub fn new_from_data(
        device: &RenderDevice,
        data: &[u8],
        stride: usize,
        usage: Option<BufferUsages>,
    ) -> Self {
        let usage = match usage {
            Some(usage) => usage | BufferUsages::VERTEX,
            None => BufferUsages::VERTEX,
        };

        Self {
            buffer: Buffer::with_data(device, data, usage, None),
            len: data.len() / stride,
        }
    }

    pub fn buffer(&self) -> &Buffer {
        &self.buffer
    }

    pub fn slice<S: RangeBounds<BufferAddress>>(&'_ self, bounds: S) -> BufferSlice<'_> {
        self.buffer.slice(bounds)
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn update<T: Pod + Zeroable>(&mut self, device: &RenderDevice, vertices: &[T]) {
        let size = vertices.len() * std::mem::size_of::<T>();
        if size > self.buffer.size() as usize {
            let usage = self.buffer.usage();
            self.buffer = Buffer::with_data(device, bytemuck::cast_slice(vertices), usage, None);
            self.len = vertices.len();
        } else {
            let data = bytemuck::cast_slice(vertices);
            device.queue.write_buffer(self.buffer.as_ref(), 0, data);
        }
    }
}
