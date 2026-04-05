use crate::{
    core::RenderDevice,
    resources::{
        ExtractError, ReadWrite, RenderAsset, VertexBufferLayout, allocator::MeshAllocator,
    },
    types::Color,
};
use asset::Asset;
use bitflags::bitflags;
use ecs::unlifetime::{Read, Write};
use math::{Vec2, Vec3, Vec4, bounds::Bounds};
use wgpu::{IndexFormat, VertexAttribute, VertexFormat};

pub mod allocator;

#[derive(
    Default, Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize,
)]
pub enum MeshTopology {
    PointList = 0,
    LineList = 1,
    LineStrip = 2,
    #[default]
    TriangleList = 3,
    TriangleStrip = 4,
}

impl Into<wgpu::PrimitiveTopology> for MeshTopology {
    fn into(self) -> wgpu::PrimitiveTopology {
        match self {
            MeshTopology::PointList => wgpu::PrimitiveTopology::PointList,
            MeshTopology::LineList => wgpu::PrimitiveTopology::LineList,
            MeshTopology::LineStrip => wgpu::PrimitiveTopology::LineStrip,
            MeshTopology::TriangleList => wgpu::PrimitiveTopology::TriangleList,
            MeshTopology::TriangleStrip => wgpu::PrimitiveTopology::TriangleStrip,
        }
    }
}

impl From<wgpu::PrimitiveTopology> for MeshTopology {
    fn from(topology: wgpu::PrimitiveTopology) -> Self {
        match topology {
            wgpu::PrimitiveTopology::PointList => MeshTopology::PointList,
            wgpu::PrimitiveTopology::LineList => MeshTopology::LineList,
            wgpu::PrimitiveTopology::LineStrip => MeshTopology::LineStrip,
            wgpu::PrimitiveTopology::TriangleList => MeshTopology::TriangleList,
            wgpu::PrimitiveTopology::TriangleStrip => MeshTopology::TriangleStrip,
        }
    }
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub enum PositionType {
    D2,
    D3,
}

impl PositionType {
    pub fn format(&self) -> VertexFormat {
        match self {
            PositionType::D2 => VertexFormat::Float32x2,
            PositionType::D3 => VertexFormat::Float32x3,
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum PositionValue {
    D2(Vec<Vec2>),
    D3(Vec<Vec3>),
}

impl PositionValue {
    pub fn ty(&self) -> PositionType {
        match self {
            PositionValue::D2(_) => PositionType::D2,
            PositionValue::D3(_) => PositionType::D3,
        }
    }

    pub fn len(&self) -> usize {
        match self {
            PositionValue::D2(v) => v.len(),
            PositionValue::D3(v) => v.len(),
        }
    }

    pub fn flag(&self) -> MeshFlags {
        match self {
            PositionValue::D2(_) => MeshFlags::POSITION_2D,
            PositionValue::D3(_) => MeshFlags::POSITION_3D,
        }
    }

    pub fn size(&self) -> usize {
        match self {
            PositionValue::D2(_) => size_of::<Vec2>(),
            PositionValue::D3(_) => size_of::<Vec3>(),
        }
    }

    pub fn data(&self) -> &[u8] {
        match self {
            PositionValue::D2(v) => bytemuck::cast_slice(v),
            PositionValue::D3(v) => bytemuck::cast_slice(v),
        }
    }
}

impl Eq for PositionValue {}
impl PartialEq for PositionValue {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::D2(_), Self::D2(_)) => true,
            (Self::D3(_), Self::D3(_)) => false,
            _ => false,
        }
    }
}

#[derive(
    Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub enum MeshAttributeType {
    Position { ty: PositionType },
    Normals,
    UV0,
    UV1,
    Tangents,
    Color,
}

impl MeshAttributeType {
    #[inline]
    pub fn format(&self) -> VertexFormat {
        match self {
            MeshAttributeType::Position { ty } => ty.format(),
            MeshAttributeType::Normals => VertexFormat::Float32x3,
            MeshAttributeType::UV0 => VertexFormat::Float32x2,
            MeshAttributeType::UV1 => VertexFormat::Float32x2,
            MeshAttributeType::Tangents => VertexFormat::Float32x4,
            MeshAttributeType::Color => VertexFormat::Float32x4,
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum MeshAttribute {
    Position(PositionValue),
    Normals(Vec<Vec3>),
    UV0(Vec<Vec2>),
    UV1(Vec<Vec2>),
    Tangents(Vec<Vec4>),
    Color(Vec<Color>),
}

impl MeshAttribute {
    pub fn ty(&self) -> MeshAttributeType {
        match self {
            MeshAttribute::Position(v) => MeshAttributeType::Position { ty: v.ty() },
            MeshAttribute::Normals(_) => MeshAttributeType::Normals,
            MeshAttribute::UV0(_) => MeshAttributeType::UV0,
            MeshAttribute::UV1(_) => MeshAttributeType::UV1,
            MeshAttribute::Tangents(_) => MeshAttributeType::Tangents,
            MeshAttribute::Color(_) => MeshAttributeType::Color,
        }
    }

    pub fn flag(&self) -> MeshFlags {
        match self {
            MeshAttribute::Position(v) => v.flag(),
            MeshAttribute::Normals(_) => MeshFlags::NORMALS,
            MeshAttribute::UV0(_) => MeshFlags::UV0,
            MeshAttribute::UV1(_) => MeshFlags::UV1,
            MeshAttribute::Tangents(_) => MeshFlags::TANGENTS,
            MeshAttribute::Color(_) => MeshFlags::COLOR,
        }
    }

    pub fn size(&self) -> usize {
        match self {
            MeshAttribute::Position(v) => v.size(),
            MeshAttribute::Normals(_) => size_of::<Vec3>(),
            MeshAttribute::UV0(_) => size_of::<Vec2>(),
            MeshAttribute::UV1(_) => size_of::<Vec2>(),
            MeshAttribute::Tangents(_) => size_of::<Vec4>(),
            MeshAttribute::Color(_) => size_of::<Vec4>(),
        }
    }

    pub fn len(&self) -> usize {
        match self {
            MeshAttribute::Position(v) => v.len(),
            MeshAttribute::Normals(v) => v.len(),
            MeshAttribute::UV0(v) => v.len(),
            MeshAttribute::UV1(v) => v.len(),
            MeshAttribute::Tangents(v) => v.len(),
            MeshAttribute::Color(v) => v.len(),
        }
    }

    pub fn data(&self) -> &[u8] {
        match self {
            MeshAttribute::Position(v) => v.data(),
            MeshAttribute::Normals(v) => bytemuck::cast_slice(v),
            MeshAttribute::UV0(v) => bytemuck::cast_slice(v),
            MeshAttribute::UV1(v) => bytemuck::cast_slice(v),
            MeshAttribute::Tangents(v) => bytemuck::cast_slice(v),
            MeshAttribute::Color(v) => bytemuck::cast_slice(v),
        }
    }
}

impl Eq for MeshAttribute {}
impl PartialEq for MeshAttribute {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Position(PositionValue::D2(_)), Self::Position(PositionValue::D2(_))) => true,
            (Self::Position(PositionValue::D3(_)), Self::Position(PositionValue::D3(_))) => true,
            (Self::Normals(_), Self::Normals(_)) => true,
            (Self::UV0(_), Self::UV0(_)) => true,
            (Self::UV1(_), Self::UV1(_)) => true,
            (Self::Tangents(_), Self::Tangents(_)) => true,
            (Self::Color(_), Self::Color(_)) => true,
            _ => false,
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum Indices {
    U16(Vec<u16>),
    U32(Vec<u32>),
}

impl Indices {
    pub fn format(&self) -> IndexFormat {
        match self {
            Indices::U16(_) => IndexFormat::Uint16,
            Indices::U32(_) => IndexFormat::Uint32,
        }
    }

    pub fn len(&self) -> usize {
        match self {
            Indices::U16(items) => items.len(),
            Indices::U32(items) => items.len(),
        }
    }

    pub fn data(&self) -> &[u8] {
        match self {
            Indices::U16(items) => bytemuck::cast_slice(items),
            Indices::U32(items) => bytemuck::cast_slice(items),
        }
    }
}

bitflags! {
    #[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
    pub struct MeshFlags: u16 {
        const POSITION_2D = 1;
        const POSITION_3D = 1 << 1;
        const NORMALS = 1 << 2;
        const UV0 = 1 << 3;
        const UV1 = 1 << 4;
        const TANGENTS = 1 << 5;
        const COLOR = 1 << 6;
    }

    #[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
    pub struct MeshDirtyFlags: u8 {
        const POSITION = 1 << 1;
        const INDICES = 1 << 2;
    }
}

#[derive(Debug, Clone, Asset, serde::Serialize, serde::Deserialize)]
pub struct Mesh {
    topology: MeshTopology,
    attributes: Vec<MeshAttribute>,
    indices: Option<Indices>,
    bounds: Bounds,
    read_write: ReadWrite,

    #[serde(skip)]
    dirty: MeshDirtyFlags,
}

impl Mesh {
    pub fn new(topology: MeshTopology) -> Self {
        Self {
            topology,
            attributes: Vec::new(),
            indices: None,
            bounds: Bounds::ZERO,
            read_write: ReadWrite::Disabled,
            dirty: MeshDirtyFlags::empty(),
        }
    }

    pub fn topology(&self) -> MeshTopology {
        self.topology
    }

    pub fn attributes(&self) -> &[MeshAttribute] {
        &self.attributes
    }

    pub fn attribute(&self, ty: MeshAttributeType) -> Option<&MeshAttribute> {
        self.index_of(ty).map(|i| &self.attributes[i])
    }

    pub fn attribute_mut(&mut self, ty: MeshAttributeType) -> Option<&mut MeshAttribute> {
        if matches!(ty, MeshAttributeType::Position { .. }) {
            self.dirty |= MeshDirtyFlags::POSITION;
        }

        self.index_of(ty).map(|i| &mut self.attributes[i])
    }

    pub fn index_of(&self, ty: MeshAttributeType) -> Option<usize> {
        self.attributes.iter().position(|a| a.ty() == ty)
    }

    pub fn indices(&self) -> Option<&Indices> {
        self.indices.as_ref()
    }

    pub fn bounds(&self) -> &Bounds {
        &self.bounds
    }

    pub fn read_write(&self) -> ReadWrite {
        self.read_write
    }

    pub fn add_attribute(&mut self, attribute: MeshAttribute) {
        if let Some(found) = self.attribute_mut(attribute.ty()) {
            *found = attribute;
        } else {
            self.attributes.push(attribute);
            self.attributes.sort_by(|a, b| a.ty().cmp(&b.ty()));
        }
    }

    pub fn set_indices(&mut self, indices: Option<Indices>) {
        self.dirty |= MeshDirtyFlags::INDICES;
        self.indices = indices;
    }

    pub fn set_read_write(&mut self, read_write: ReadWrite) {
        self.read_write = read_write;
    }

    pub fn vertices(&self) -> MeshVertexInfo {
        let mut count = self.attributes.first().map(|a| a.len()).unwrap_or(0);
        let mut size = 0usize;
        let mut flags = MeshFlags::empty();

        for attribute in &self.attributes {
            size += attribute.size();
            count = attribute.len().min(count);
            flags |= attribute.flag();
        }

        let mut vertices = Vec::with_capacity(size * count);
        let mut offset = 0;

        for index in 0..self.attributes.len() {
            let data = self.attributes[index].data();
            let size = self.attributes[index].size();
            for index in 0..count {
                let start = index * size;
                let src = &data[start..start + size];

                let start = index * size + offset;
                let dst = &mut vertices[start..start + size];

                dst.copy_from_slice(src);
            }

            offset += size;
        }

        MeshVertexInfo {
            vertices,
            size,
            count,
            flags,
        }
    }
}

pub struct MeshVertexInfo {
    pub vertices: Vec<u8>,
    pub size: usize,
    pub count: usize,
    pub flags: MeshFlags,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum MeshFormat {
    NonIndexed,
    Indexed {
        format: IndexFormat,
        index_count: u32,
    },
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct GpuMesh {
    pub flags: MeshFlags,
    pub format: MeshFormat,
    pub vertex_count: u32,
}

impl RenderAsset for GpuMesh {
    type Asset = Mesh;

    type Arg = (Read<RenderDevice>, Write<MeshAllocator>);

    fn extract(
        id: asset::AssetId<Self::Asset>,
        asset: Self::Asset,
        (device, allocator): &mut ecs::ArgItem<Self::Arg>,
    ) -> Result<Self, ExtractError<Self::Asset>> {
        allocator
            .allocate(device, &id, &asset)
            .ok_or(ExtractError::from_str("Failed to extract mesh."))
    }

    fn removed(
        id: &asset::AssetId<Self::Asset>,
        _: &Self,
        (_, allocator): &mut ecs::ArgItem<Self::Arg>,
    ) {
        allocator.release(id);
    }

    fn usage(asset: &Self::Asset) -> super::AssetUsage {
        match asset.read_write() {
            ReadWrite::Enabled => super::AssetUsage::Keep,
            ReadWrite::Disabled => super::AssetUsage::Discard,
        }
    }
}

impl<T: IntoIterator<Item = MeshAttributeType>> From<T> for VertexBufferLayout {
    fn from(value: T) -> Self {
        let mut attributes = Vec::new();
        let mut stride = 0;

        for ty in value {
            let format = ty.format();
            attributes.push(VertexAttribute {
                format,
                offset: stride,
                shader_location: attributes.len() as u32,
            });
            stride += format.size();
        }

        VertexBufferLayout {
            array_stride: stride,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes,
        }
    }
}
