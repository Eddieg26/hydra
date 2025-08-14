use crate::{Buffer, ExtractInfo, Mesh, RenderDevice, RenderMesh};
use asset::AssetId;
use ecs::Resource;
use offset_allocator::{Allocation, Allocator};
use std::{
    collections::{HashMap, HashSet},
    ops::Range,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChunkUsage {
    /// Indicates that the chunk is used for vertex data.
    Vertex,
    /// Indicates that the chunk is used for index data.
    Index,
}

impl From<ChunkUsage> for wgpu::BufferUsages {
    fn from(value: ChunkUsage) -> Self {
        match value {
            ChunkUsage::Vertex => {
                wgpu::BufferUsages::VERTEX
                    | wgpu::BufferUsages::COPY_DST
                    | wgpu::BufferUsages::COPY_SRC
            }
            ChunkUsage::Index => {
                wgpu::BufferUsages::INDEX
                    | wgpu::BufferUsages::COPY_DST
                    | wgpu::BufferUsages::COPY_SRC
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ChunkMeta {
    /// The usage of the chunk, indicating whether it is for vertex or index data.
    usage: ChunkUsage,
    /// The size of the chunk in bytes.
    size: u32,
    /// The alignment of the chunk, ensuring it meets the requirements for GPU memory.
    /// This is typically aligned to the GPU's copy buffer alignment.
    alignment: u32,
}

impl ChunkMeta {
    pub fn new(usage: ChunkUsage, size: u32) -> Self {
        let alignment = Self::align(size);

        Self {
            usage,
            size,
            alignment,
        }
    }

    fn align(size: u32) -> u32 {
        use wgpu::COPY_BUFFER_ALIGNMENT;

        (size + COPY_BUFFER_ALIGNMENT as u32 - 1) & !(COPY_BUFFER_ALIGNMENT as u32 - 1)
    }
}

#[derive(Resource)]
pub struct MeshAllocatorConfig {
    /// The minimum size of a chunk in bytes.
    ///
    /// Defaults to 1MB.
    pub min_chunk_size: u32,

    /// The maximum size of a chunk in bytes.
    ///
    /// Defaults to 512MB.
    pub max_chunk_size: u32,

    /// The threshold for when to use a block allocator instead of an inline allocator.
    ///
    /// Defaults to 256MB.
    pub block_threshold: u32,
}

impl Default for MeshAllocatorConfig {
    fn default() -> Self {
        Self {
            min_chunk_size: 1 * 1024 * 1024,    // 1MB
            max_chunk_size: 512 * 1024 * 1024,  // 512MB
            block_threshold: 256 * 1024 * 1024, // 256MB
        }
    }
}

#[derive(Clone, Copy)]
pub struct MeshAllocation {
    allocation: Allocation,

    size: u32,
}

impl std::ops::Deref for MeshAllocation {
    type Target = Allocation;

    fn deref(&self) -> &Self::Target {
        &self.allocation
    }
}

pub struct PendingAllocation {
    id: AssetId<Mesh>,
    allocation: MeshAllocation,
    data: Vec<u8>,
}

#[derive(Default)]
pub struct PendingAllocations {
    allocations: Vec<PendingAllocation>,
    size: u64,
}

impl PendingAllocations {
    pub fn with(id: AssetId<Mesh>, allocation: MeshAllocation, data: Vec<u8>) -> Self {
        let size = allocation.offset + allocation.size;
        Self {
            allocations: vec![PendingAllocation {
                id,
                allocation,
                data,
            }],
            size: size as u64,
        }
    }

    pub fn push(&mut self, id: AssetId<Mesh>, allocation: MeshAllocation, data: Vec<u8>) {
        self.size = self.size.max((allocation.offset + allocation.size) as u64);
        self.allocations.push(PendingAllocation {
            id,
            allocation,
            data,
        });
    }
}

pub struct InlineChunk {
    /// The allocator used for managing allocations within this chunk.
    allocator: Allocator,

    /// A map of allocations, keyed by the asset ID of the mesh.
    allocations: HashMap<AssetId<Mesh>, MeshAllocation>,

    /// The buffer that holds the mesh data for this chunk.
    buffer: Buffer,

    /// Metadata about the chunk, including its usage and size.
    meta: ChunkMeta,
}

impl InlineChunk {
    pub fn new(device: &RenderDevice, meta: ChunkMeta, size: u64, capacity: u32) -> Self {
        Self {
            allocator: Allocator::new(capacity),
            allocations: HashMap::new(),
            buffer: Buffer::new(device, size, meta.usage.into(), None),
            meta,
        }
    }

    pub fn allocate(&mut self, id: AssetId<Mesh>, size: usize) -> Option<MeshAllocation> {
        let size = ChunkMeta::align(size as u32);
        let allocation = MeshAllocation {
            allocation: self.allocator.allocate(size)?,
            size,
        };

        self.allocations.insert(id, allocation);
        Some(allocation)
    }

    pub fn free(&mut self, id: &AssetId<Mesh>) -> bool {
        if let Some(allocation) = self.allocations.remove(id) {
            self.allocator.free(*allocation);
            true
        } else {
            false
        }
    }
}

pub struct BlockChunk {
    /// The buffer that holds the mesh data for this chunk.
    buffer: Buffer,

    /// Metadata about the chunk, including its usage and size.
    meta: ChunkMeta,
}

impl BlockChunk {
    pub fn new(device: &RenderDevice, size: u32, meta: ChunkMeta) -> Self {
        let buffer = Buffer::new(device, size as u64, meta.usage.into(), None);
        Self { buffer, meta }
    }
}

pub enum MeshChunk {
    Inline(InlineChunk),
    Block(BlockChunk),
}

impl MeshChunk {
    pub fn usage(&self) -> ChunkUsage {
        match self {
            MeshChunk::Inline(chunk) => chunk.meta.usage,
            MeshChunk::Block(chunk) => chunk.meta.usage,
        }
    }

    pub fn buffer(&self) -> &Buffer {
        match self {
            MeshChunk::Inline(chunk) => &chunk.buffer,
            MeshChunk::Block(chunk) => &chunk.buffer,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ChunkId(usize);

#[derive(Resource)]
pub struct MeshAllocator {
    /// A map of chunk IDs to their corresponding mesh chunks.
    chunks: HashMap<ChunkId, MeshChunk>,

    /// A map of chunk metadata to a list of chunk IDs that match that metadata.
    metas: HashMap<ChunkMeta, Vec<ChunkId>>,

    /// A map of mesh asset IDs to the chunk ID that contains their vertex data.
    vertex_chunks: HashMap<AssetId<Mesh>, ChunkId>,

    /// A map of mesh asset IDs to the chunk ID that contains their index data.
    index_chunks: HashMap<AssetId<Mesh>, ChunkId>,

    /// The next chunk ID to be assigned.
    next_chunk_id: usize,

    /// Configuration for the mesh allocator, including chunk sizes and thresholds.
    config: MeshAllocatorConfig,
}

impl MeshAllocator {
    pub fn new(config: MeshAllocatorConfig) -> Self {
        Self {
            chunks: HashMap::new(),
            metas: HashMap::new(),
            vertex_chunks: HashMap::new(),
            index_chunks: HashMap::new(),
            next_chunk_id: 0,
            config,
        }
    }

    pub fn vertex_slice(&self, id: &AssetId<Mesh>) -> Option<MeshBufferSlice<'_>> {
        self.buffer_slice(id, self.vertex_chunks.get(id)?)
    }

    pub fn index_slice(&self, id: &AssetId<Mesh>) -> Option<MeshBufferSlice<'_>> {
        self.buffer_slice(id, self.index_chunks.get(id)?)
    }

    fn buffer_slice(&self, id: &AssetId<Mesh>, chunk_id: &ChunkId) -> Option<MeshBufferSlice<'_>> {
        let chunk = self.chunks.get(chunk_id)?;
        match chunk {
            MeshChunk::Inline(chunk) => {
                let alloc = chunk.allocations.get(id)?;
                let range = alloc.offset / chunk.meta.alignment
                    ..(alloc.offset + alloc.size) / chunk.meta.alignment;
                Some(MeshBufferSlice {
                    buffer: &chunk.buffer,
                    range,
                })
            }
            MeshChunk::Block(chunk) => {
                let range = 0..(chunk.buffer.size() as u32).div_ceil(chunk.meta.alignment);
                Some(MeshBufferSlice {
                    buffer: &chunk.buffer,
                    range,
                })
            }
        }
    }

    pub fn allocate<'a>(
        &mut self,
        device: &RenderDevice,
        meshes: impl Iterator<Item = &'a (AssetId<Mesh>, Mesh)>,
    ) {
        let mut pending = HashMap::new();
        for (id, mesh) in meshes {
            let meta = ChunkMeta::new(ChunkUsage::Vertex, mesh.vertex_size() as u32);
            let data = mesh.vertex_data().0;
            self.queue(device, id, data, meta, &mut pending);

            if let Some(indices) = mesh.indices() {
                let meta = ChunkMeta::new(ChunkUsage::Index, indices.format().byte_size() as u32);
                let data = indices.data().to_vec();
                self.queue(device, id, data, meta, &mut pending);
            }
        }

        self.allocate_inner(device, pending);
    }

    fn queue(
        &mut self,
        device: &RenderDevice,
        id: &AssetId<Mesh>,
        data: Vec<u8>,
        meta: ChunkMeta,
        pending: &mut HashMap<ChunkId, PendingAllocations>,
    ) {
        let alloc_size = ChunkMeta::align(data.len() as u32);
        if alloc_size < self.config.block_threshold {
            let result = self.metas.get_mut(&meta).iter_mut().find_map(|ids| {
                ids.iter_mut().find_map(|chunk_id| {
                    let MeshChunk::Inline(chunk) = self.chunks.get_mut(&chunk_id)? else {
                        return None;
                    };

                    chunk
                        .allocate(*id, alloc_size as usize)
                        .map(|allocation| (*chunk_id, allocation))
                })
            });

            if let Some((chunk, allocation)) = result {
                pending
                    .entry(chunk)
                    .or_default()
                    .push(*id, allocation, data);
            } else {
                let size = (alloc_size as u64).max(self.config.min_chunk_size as u64);
                let mut chunk = InlineChunk::new(device, meta, size, self.config.max_chunk_size);
                if let Some(allocation) = chunk.allocate(*id, alloc_size as usize) {
                    let chunk_id = ChunkId(self.next_chunk_id);
                    pending.insert(chunk_id, PendingAllocations::with(*id, allocation, data));
                    self.chunks.insert(chunk_id, MeshChunk::Inline(chunk));
                    self.metas.entry(meta).or_default().push(chunk_id);
                    self.next_chunk_id += 1;
                }
            }
        } else if let Some(allocation) = Allocator::new(alloc_size).allocate(alloc_size) {
            let chunk = BlockChunk::new(device, alloc_size, meta);
            let chunk_id = ChunkId(self.next_chunk_id);
            let allocation = MeshAllocation {
                allocation,
                size: alloc_size,
            };
            pending.insert(chunk_id, PendingAllocations::with(*id, allocation, data));
            self.chunks.insert(chunk_id, MeshChunk::Block(chunk));
            self.metas.entry(meta).or_default().push(chunk_id);
            self.next_chunk_id += 1;
        }
    }

    fn allocate_inner(
        &mut self,
        device: &RenderDevice,
        pending: HashMap<ChunkId, PendingAllocations>,
    ) {
        for (chunk_id, pending) in pending {
            let Some(chunk) = self.chunks.get_mut(&chunk_id) else {
                continue;
            };

            if let MeshChunk::Inline(chunk) = chunk {
                if chunk.buffer.size() < pending.size {
                    let buffer = Buffer::new(device, pending.size, chunk.meta.usage.into(), None);
                    let mut encoder = device.create_command_encoder(&Default::default());

                    encoder.copy_buffer_to_buffer(
                        chunk.buffer.as_ref(),
                        0,
                        buffer.as_ref(),
                        0,
                        chunk.buffer.size(),
                    );

                    chunk.buffer = buffer;

                    device.queue.submit(std::iter::once(encoder.finish()));
                }
            }

            for pending in pending.allocations {
                device.queue.write_buffer(
                    chunk.buffer().as_ref(),
                    pending.allocation.offset as u64,
                    &pending.data,
                );

                match chunk.usage() {
                    ChunkUsage::Vertex => self.vertex_chunks.insert(pending.id, chunk_id),
                    ChunkUsage::Index => self.index_chunks.insert(pending.id, chunk_id),
                };
            }
        }
    }

    pub fn release(&mut self, ids: &HashSet<AssetId<Mesh>>) {
        for id in ids {
            if let Some(chunk) = self.free(id, ChunkUsage::Vertex) {
                self.chunks.remove(&chunk);
            }

            if let Some(chunk) = self.free(id, ChunkUsage::Index) {
                self.chunks.remove(&chunk);
            }
        }
    }

    fn free(&mut self, id: &AssetId<Mesh>, usage: ChunkUsage) -> Option<ChunkId> {
        let chunk = match usage {
            ChunkUsage::Vertex => self.vertex_chunks.remove(id),
            ChunkUsage::Index => self.index_chunks.remove(id),
        }?;

        match self.chunks.get_mut(&chunk)? {
            MeshChunk::Inline(inline) => inline.free(id).then_some(chunk),
            MeshChunk::Block(_) => Some(chunk),
        }
    }

    pub(crate) fn update(allocator: &mut Self, device: &RenderDevice, extracted: &ExtractInfo<RenderMesh>) {
        allocator.release(&extracted.removed);
        allocator.allocate(device, extracted.extracted.iter());
    }
}

pub struct MeshBufferSlice<'a> {
    /// The buffer that contains the mesh data.
    pub buffer: &'a Buffer,

    /// The range of the slice in elements.
    pub range: Range<u32>,
}

impl<'a> MeshBufferSlice<'a> {
    pub fn new(buffer: &'a Buffer, range: Range<u32>) -> Self {
        Self { buffer, range }
    }
}
