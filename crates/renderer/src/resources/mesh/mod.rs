use crate::{resources::ReadWrite, types::Color};
use asset::Asset;
use bitflags::bitflags;
use math::{Vec2, Vec3, Vec4, bounds::Bounds};

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

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum PositionType {
    D2(Vec<Vec2>),
    D3(Vec<Vec3>),
}

impl PositionType {
    pub fn len(&self) -> usize {
        match self {
            PositionType::D2(v) => v.len(),
            PositionType::D3(v) => v.len(),
        }
    }

    pub fn flag(&self) -> MeshFlags {
        match self {
            PositionType::D2(_) => MeshFlags::POSITION_2D,
            PositionType::D3(_) => MeshFlags::POSITION_3D,
        }
    }

    pub fn size(&self) -> usize {
        match self {
            PositionType::D2(_) => size_of::<Vec2>(),
            PositionType::D3(_) => size_of::<Vec3>(),
        }
    }

    pub fn data(&self) -> &[u8] {
        match self {
            PositionType::D2(v) => bytemuck::cast_slice(v),
            PositionType::D3(v) => bytemuck::cast_slice(v),
        }
    }
}

impl Eq for PositionType {}
impl PartialEq for PositionType {
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
    Position,
    Normals,
    UV0,
    UV1,
    Tangents,
    Color,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum MeshAttribute {
    Position(PositionType),
    Normals(Vec<Vec3>),
    UV0(Vec<Vec2>),
    UV1(Vec<Vec2>),
    Tangents(Vec<Vec4>),
    Color(Vec<Color>),
}

impl MeshAttribute {
    pub fn ty(&self) -> MeshAttributeType {
        match self {
            MeshAttribute::Position(_) => MeshAttributeType::Position,
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
            (Self::Position(PositionType::D2(_)), Self::Position(PositionType::D2(_))) => true,
            (Self::Position(PositionType::D3(_)), Self::Position(PositionType::D3(_))) => true,
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

bitflags! {
    #[derive(Debug, Default, Clone, Copy)]
    pub struct MeshFlags: u16 {
        const POSITION_2D = 1;
        const POSITION_3D = 1 << 1;
        const NORMALS = 1 << 2;
        const UV0 = 1 << 3;
        const UV1 = 1 << 4;
        const TANGENTS = 1 << 5;
        const COLOR = 1 << 6;
    }

    #[derive(Debug, Default, Clone, Copy)]
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
        if ty == MeshAttributeType::Position {
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

    pub fn set_indices(&mut self, indices: Indices) {
        self.dirty |= MeshDirtyFlags::INDICES;
        self.indices = Some(indices);
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
