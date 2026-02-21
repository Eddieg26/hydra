use super::{
    VertexBufferLayout,
    buffer::Indices,
    extract::{ExtractError, ReadWrite, RenderAsset},
};
use crate::{Aabb, RenderAssetType, primitives::Color};
use asset::{Asset, AssetId};
use ecs::{Component, IndexMap, system::ArgItem};
use std::{hash::Hash, ops::Range};
use wgpu::{IndexFormat, VertexStepMode};

pub mod allocator;

#[derive(
    Copy, Clone, Debug, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
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

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum MeshAttributeValues {
    Float(Vec<f32>),
    Vec2(Vec<math::Vec2>),
    Vec3(Vec<math::Vec3>),
    Vec4(Vec<math::Vec4>),
}

impl MeshAttributeValues {
    pub fn len(&self) -> usize {
        match self {
            MeshAttributeValues::Float(v) => v.len(),
            MeshAttributeValues::Vec2(v) => v.len(),
            MeshAttributeValues::Vec3(v) => v.len(),
            MeshAttributeValues::Vec4(v) => v.len(),
        }
    }

    pub fn extend(&mut self, other: &Self) {
        match (self, other) {
            (MeshAttributeValues::Float(a), MeshAttributeValues::Float(b)) => a.extend(b),
            (MeshAttributeValues::Vec2(a), MeshAttributeValues::Vec2(b)) => a.extend(b),
            (MeshAttributeValues::Vec3(a), MeshAttributeValues::Vec3(b)) => a.extend(b),
            (MeshAttributeValues::Vec4(a), MeshAttributeValues::Vec4(b)) => a.extend(b),
            _ => (),
        }
    }

    pub fn is_empty(&self) -> bool {
        match self {
            MeshAttributeValues::Float(v) => v.is_empty(),
            MeshAttributeValues::Vec2(v) => v.is_empty(),
            MeshAttributeValues::Vec3(v) => v.is_empty(),
            MeshAttributeValues::Vec4(v) => v.is_empty(),
        }
    }

    pub fn data(&self, range: Range<usize>) -> &[u8] {
        match self {
            MeshAttributeValues::Float(v) => bytemuck::cast_slice(&v[range]),
            MeshAttributeValues::Vec2(v) => bytemuck::cast_slice(&v[range]),
            MeshAttributeValues::Vec3(v) => bytemuck::cast_slice(&v[range]),
            MeshAttributeValues::Vec4(v) => bytemuck::cast_slice(&v[range]),
        }
    }

    pub fn format(&self) -> wgpu::VertexFormat {
        match self {
            MeshAttributeValues::Float(_) => wgpu::VertexFormat::Float32,
            MeshAttributeValues::Vec2(_) => wgpu::VertexFormat::Float32x2,
            MeshAttributeValues::Vec3(_) => wgpu::VertexFormat::Float32x3,
            MeshAttributeValues::Vec4(_) => wgpu::VertexFormat::Float32x4,
        }
    }

    pub fn size(&self) -> usize {
        match self {
            MeshAttributeValues::Float(_) => std::mem::size_of::<f32>(),
            MeshAttributeValues::Vec2(_) => std::mem::size_of::<math::Vec2>(),
            MeshAttributeValues::Vec3(_) => std::mem::size_of::<math::Vec3>(),
            MeshAttributeValues::Vec4(_) => std::mem::size_of::<math::Vec4>(),
        }
    }

    pub fn clear(&mut self) {
        match self {
            MeshAttributeValues::Float(v) => v.clear(),
            MeshAttributeValues::Vec2(v) => v.clear(),
            MeshAttributeValues::Vec3(v) => v.clear(),
            MeshAttributeValues::Vec4(v) => v.clear(),
        }
    }
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub enum MeshAttributeType {
    Position,
    Normal,
    TexCoord0,
    TexCoord1,
    Tangent,
    Color,
}

impl MeshAttributeType {
    pub fn size(&self) -> usize {
        match self {
            MeshAttributeType::Position => std::mem::size_of::<math::Vec3>(),
            MeshAttributeType::Normal => std::mem::size_of::<math::Vec3>(),
            MeshAttributeType::TexCoord0 => std::mem::size_of::<math::Vec2>(),
            MeshAttributeType::TexCoord1 => std::mem::size_of::<math::Vec2>(),
            MeshAttributeType::Tangent => std::mem::size_of::<math::Vec4>(),
            MeshAttributeType::Color => std::mem::size_of::<Color>(),
        }
    }
}

impl Iterator for MeshAttributeType {
    type Item = Self;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            MeshAttributeType::Position => Some(MeshAttributeType::Normal),
            MeshAttributeType::Normal => Some(MeshAttributeType::TexCoord0),
            MeshAttributeType::TexCoord0 => Some(MeshAttributeType::TexCoord1),
            MeshAttributeType::TexCoord1 => Some(MeshAttributeType::Tangent),
            MeshAttributeType::Tangent => Some(MeshAttributeType::Color),
            MeshAttributeType::Color => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MeshAttribute {
    pub ty: MeshAttributeType,
    pub values: MeshAttributeValues,
}

impl MeshAttribute {
    pub fn new(ty: MeshAttributeType, values: MeshAttributeValues) -> Self {
        Self { ty, values }
    }
}

bitflags::bitflags! {
    #[derive(Default, Clone, Copy, PartialEq, Eq)]
    pub struct MeshDirty: u32 {
        const POSITION = 1 << 1;
        const NORMAL = 1 << 2;
        const TANGENT =  1 << 3;
        const TEXCOORD0 = 1 << 4;
        const TEXCOORD1 = 1 << 5;
        const COLOR = 1 << 6;
        const INDICES = 1 << 7;
        const BOUNDS = 1 << 8;
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, Asset, serde::Serialize, serde::Deserialize)]
pub struct SubMesh {
    pub start_vertex: u32,
    pub vertex_count: u32,
    pub start_index: u32,
    pub index_count: u32,
}

impl SubMesh {
    pub fn new(start_vertex: u32, vertex_count: u32, start_index: u32, index_count: u32) -> Self {
        Self {
            start_vertex,
            vertex_count,
            start_index,
            index_count,
        }
    }
}

impl From<&Mesh> for SubMesh {
    fn from(mesh: &Mesh) -> Self {
        Self {
            start_vertex: 0,
            vertex_count: mesh.vertex_count() as u32,
            start_index: 0,
            index_count: mesh.index_count() as u32,
        }
    }
}

impl RenderAsset for SubMesh {
    type Source = Self;

    type Arg = ();

    fn extract(
        _: AssetId<Self::Source>,
        asset: Self::Source,
        _arg: &mut ArgItem<Self::Arg>,
    ) -> Result<Self, ExtractError<Self::Source>> {
        Ok(asset)
    }

    fn dependencies() -> Vec<super::RenderAssetType> {
        vec![RenderAssetType::of::<RenderMesh>()]
    }
}

#[derive(Clone, Asset, serde::Serialize, serde::Deserialize)]
pub struct Mesh {
    topology: MeshTopology,
    attributes: Vec<MeshAttribute>,
    indices: Option<Indices>,
    bounds: Aabb,
    read_write: ReadWrite,

    #[serde(skip)]
    dirty: MeshDirty,
}

impl Mesh {
    pub fn new(topology: MeshTopology) -> Self {
        Self {
            topology,
            attributes: Vec::new(),
            indices: None,
            bounds: Aabb::ZERO,
            read_write: ReadWrite::Disabled,
            dirty: MeshDirty::empty(),
        }
    }

    pub fn with_attribute(mut self, attribute: MeshAttribute) -> Self {
        self.add_attribute(attribute);
        self
    }

    pub fn with_indices(mut self, indices: Indices) -> Self {
        self.set_indices(indices);
        self
    }

    pub fn with_read_write(mut self, read_write: ReadWrite) -> Self {
        self.read_write = read_write;
        self
    }

    pub fn topology(&self) -> MeshTopology {
        self.topology
    }

    pub fn attributes(&self) -> &[MeshAttribute] {
        &self.attributes
    }

    pub fn attribute(&self, kind: MeshAttributeType) -> Option<&MeshAttribute> {
        self.attribute_index(kind).map(|i| &self.attributes[i])
    }

    pub fn attribute_mut(&mut self, kind: MeshAttributeType) -> Option<&mut MeshAttribute> {
        match self.attribute_index(kind) {
            Some(i) => {
                self.attribute_dirty(kind);
                Some(&mut self.attributes[i])
            }
            None => None,
        }
    }

    pub fn has_attribute(&self, kind: MeshAttributeType) -> bool {
        self.attribute_index(kind).is_some()
    }

    pub fn dirty(&self) -> MeshDirty {
        self.dirty
    }

    pub fn indices(&self) -> Option<&Indices> {
        self.indices.as_ref()
    }

    pub fn indices_mut(&mut self) -> Option<&mut Indices> {
        let indices = self.indices.as_mut();

        if indices.is_some() {
            self.dirty |= MeshDirty::INDICES;
        }

        indices
    }

    pub fn bounds(&self) -> Aabb {
        self.bounds
    }

    pub fn read_write(&self) -> ReadWrite {
        self.read_write
    }

    pub fn add_attribute(&mut self, attribute: MeshAttribute) -> &mut Self {
        let ty = attribute.ty;
        match self.attribute_index(ty) {
            Some(i) => self.attributes[i] = attribute,
            None => self.attributes.push(attribute),
        }

        self.attribute_dirty(ty);

        self
    }

    pub fn remove_attribute(&mut self, ty: MeshAttributeType) -> Option<MeshAttribute> {
        let removed = self.attribute_index(ty).map(|i| self.attributes.remove(i));

        self.attribute_dirty(ty);

        removed
    }

    pub fn set_indices(&mut self, indices: Indices) {
        self.indices = Some(indices);
        self.dirty |= MeshDirty::INDICES;
    }

    pub fn add_indices(&mut self, indices: Indices) {
        match self.indices {
            Some(ref mut i) => i.extend(&indices),
            None => self.indices = Some(indices),
        }
    }

    pub fn attribute_index(&self, ty: MeshAttributeType) -> Option<usize> {
        self.attributes.iter().position(|a| a.ty == ty)
    }

    pub fn clear(&mut self) {
        for attribute in &mut self.attributes {
            attribute.values.clear();
        }

        self.indices = None;
        self.dirty = MeshDirty::all()
    }

    pub fn vertex_count(&self) -> u64 {
        if self.attributes.is_empty() {
            return 0;
        }

        self.attributes
            .iter()
            .fold(usize::MAX, |len, curr| len.min(curr.values.len())) as u64
    }

    pub fn vertex_size(&self) -> u64 {
        self.attributes
            .iter()
            .fold(0, |size, attr| size + attr.values.size()) as u64
    }

    pub fn index_count(&self) -> usize {
        self.indices.as_ref().map_or(0, |i| i.len())
    }

    pub fn calculate_bounds(&mut self) {
        let bounds_dirty = self.dirty.contains(MeshDirty::BOUNDS);
        let Some(attribute) = self.attribute(MeshAttributeType::Position) else {
            return;
        };

        match (bounds_dirty, &attribute.values) {
            (true, MeshAttributeValues::Vec3(positions)) => {
                self.bounds = Aabb::from(positions.as_slice());
                self.dirty.remove(MeshDirty::BOUNDS);
            }
            (true, MeshAttributeValues::Vec2(positions)) => {
                self.bounds = Aabb::from(positions.as_slice());
                self.dirty.remove(MeshDirty::BOUNDS);
            }
            (true, MeshAttributeValues::Vec4(positions)) => {
                self.bounds = Aabb::from(positions.as_slice());
                self.dirty.remove(MeshDirty::BOUNDS);
            }
            _ => (),
        }
    }

    pub fn attribute_data(&self, kind: MeshAttributeType, range: Range<usize>) -> &[u8] {
        self.attribute(kind).map_or(&[], |a| a.values.data(range))
    }

    pub fn attribute_dirty(&mut self, attribute: MeshAttributeType) {
        match attribute {
            MeshAttributeType::Position => self.dirty |= MeshDirty::POSITION | MeshDirty::BOUNDS,
            MeshAttributeType::Normal => self.dirty |= MeshDirty::NORMAL,
            MeshAttributeType::Tangent => self.dirty |= MeshDirty::TANGENT,
            MeshAttributeType::TexCoord0 => self.dirty |= MeshDirty::TEXCOORD0,
            MeshAttributeType::TexCoord1 => self.dirty |= MeshDirty::TEXCOORD1,
            MeshAttributeType::Color => self.dirty |= MeshDirty::COLOR,
        }
    }

    pub fn is_attribute_dirty(&self, attribute: MeshAttributeType) -> bool {
        match attribute {
            MeshAttributeType::Position => self.dirty.contains(MeshDirty::POSITION),
            MeshAttributeType::Normal => self.dirty.contains(MeshDirty::NORMAL),
            MeshAttributeType::Tangent => self.dirty.contains(MeshDirty::TANGENT),
            MeshAttributeType::TexCoord0 => self.dirty.contains(MeshDirty::TEXCOORD0),
            MeshAttributeType::TexCoord1 => self.dirty.contains(MeshDirty::TEXCOORD1),
            MeshAttributeType::Color => self.dirty.contains(MeshDirty::COLOR),
        }
    }

    pub fn flags(&self) -> MeshFlags {
        let mut flags = MeshFlags::empty();
        for attribute in &self.attributes {
            match attribute.ty {
                MeshAttributeType::Position => flags.set(MeshFlags::POSITION, true),
                MeshAttributeType::Normal => flags.set(MeshFlags::NORMAL, true),
                MeshAttributeType::TexCoord0 => flags.set(MeshFlags::UV0, true),
                MeshAttributeType::TexCoord1 => flags.set(MeshFlags::UV1, true),
                MeshAttributeType::Tangent => flags.set(MeshFlags::TANGENT, true),
                MeshAttributeType::Color => flags.set(MeshFlags::COLOR, true),
            }
        }

        flags
    }

    pub fn layout(&self) -> MeshLayout {
        MeshLayout::from(
            self.attributes
                .iter()
                .map(|a| MeshAttributeLayout {
                    ty: a.ty,
                    format: a.values.format(),
                })
                .collect::<Vec<_>>(),
        )
    }

    pub fn vertex_data(&self) -> (Vec<u8>, usize) {
        let count = self.vertex_count() as usize;
        let mut data = vec![];

        for index in 0..count {
            for attribute in &self.attributes {
                match &attribute.values {
                    MeshAttributeValues::Float(v) => {
                        data.extend_from_slice(bytemuck::bytes_of(&v[index]))
                    }
                    MeshAttributeValues::Vec2(v) => {
                        data.extend_from_slice(bytemuck::bytes_of(&v[index]))
                    }
                    MeshAttributeValues::Vec3(v) => {
                        data.extend_from_slice(bytemuck::bytes_of(&v[index]))
                    }
                    MeshAttributeValues::Vec4(v) => {
                        data.extend_from_slice(bytemuck::bytes_of(&v[index]))
                    }
                }
            }
        }

        (data, count)
    }

    pub fn create_render_mesh(&mut self) -> RenderMesh {
        let attributes = self
            .attributes()
            .iter()
            .map(|a| MeshAttributeLayout {
                ty: a.ty,
                format: a.values.format(),
            })
            .collect::<Vec<_>>();

        let layout = MeshLayout::from(attributes);
        let key = MeshFlags::from(&layout);

        self.calculate_bounds();

        RenderMesh {
            flags: key,
            layout: layout.into(),
            vertex_count: self.vertex_count() as u32,
            format: match self.indices {
                Some(ref indices) => MeshFormat::Indexed {
                    format: indices.format(),
                    count: indices.len() as u32,
                },
                None => MeshFormat::NonIndexed,
            },
            bounds: self.bounds,
        }
    }

    pub fn calculate_normals(positions: &[math::Vec3], indices: &[u32]) -> Vec<math::Vec3> {
        let mut normals = vec![math::Vec3::ONE; positions.len()];

        for chunk in indices.chunks(3) {
            if chunk.len() < 3 {
                continue;
            }

            let a = positions[chunk[0] as usize];
            let b = positions[chunk[1] as usize];
            let c = positions[chunk[2] as usize];

            let edge1 = b - a;
            let edge2 = c - a;
            let cross = edge1.cross(edge2);

            if cross.length_squared() < 1e-6 {
                continue; // Skip degenerate
            }

            let normal = cross.normalize();

            normals[chunk[0] as usize] += normal;
            normals[chunk[1] as usize] += normal;
            normals[chunk[2] as usize] += normal;
        }

        for normal in &mut normals {
            *normal = normal.normalize();
        }

        normals
    }

    pub fn build_indices(positions: &[math::Vec3]) -> Vec<u32> {
        #[derive(Hash, Eq, PartialEq, Clone, Copy)]
        struct VertexKey {
            x: i32,
            y: i32,
            z: i32,
        }

        impl From<&math::Vec3> for VertexKey {
            fn from(vec: &math::Vec3) -> Self {
                const SCALE: f32 = 1000.0; // Scale to avoid floating point precision issues

                Self {
                    x: (vec.x * SCALE) as i32,
                    y: (vec.y * SCALE) as i32,
                    z: (vec.z * SCALE) as i32,
                }
            }
        }

        let mut keys = IndexMap::new();
        let mut indices = Vec::new();

        for (i, position) in positions.iter().enumerate() {
            let key = VertexKey::from(position);
            if let Some(&index) = keys.get(&key) {
                indices.push(index as u32);
            } else {
                keys.insert(key, i);
                indices.push(i as u32);
            }
        }

        indices
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MeshAttributeLayout {
    pub ty: MeshAttributeType,
    pub format: wgpu::VertexFormat,
}

impl MeshAttributeLayout {
    pub const fn new(ty: MeshAttributeType, format: wgpu::VertexFormat) -> Self {
        Self { ty, format }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeshLayout(Box<[MeshAttributeLayout]>);

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct MeshFlags: u64 {
        const POSITION      = 1 << 0;
        const NORMAL        = 1 << 1;
        const UV0           = 1 << 2;
        const UV1           = 1 << 3;
        const TANGENT       = 1 << 4;
        const COLOR         = 1 << 5;
        const SKINNING      = 1 << 6;
    }
}

impl From<&MeshLayout> for MeshFlags {
    fn from(value: &MeshLayout) -> Self {
        let mut key = MeshFlags::empty();
        for attribute in &value.0 {
            match attribute.ty {
                MeshAttributeType::Position => key.set(MeshFlags::POSITION, true),
                MeshAttributeType::Normal => key.set(MeshFlags::NORMAL, true),
                MeshAttributeType::TexCoord0 => key.set(MeshFlags::UV0, true),
                MeshAttributeType::TexCoord1 => key.set(MeshFlags::UV1, true),
                MeshAttributeType::Tangent => key.set(MeshFlags::TANGENT, true),
                MeshAttributeType::Color => key.set(MeshFlags::COLOR, true),
            }
        }

        key
    }
}

impl MeshLayout {
    pub fn into_vertex_buffer_layout<'a>(
        start_location: u32,
        formats: impl IntoIterator<Item = &'a wgpu::VertexFormat>,
        mode: VertexStepMode,
    ) -> VertexBufferLayout {
        let mut stride = 0;
        let mut attributes = vec![];

        for (i, format) in formats.into_iter().enumerate() {
            attributes.push(wgpu::VertexAttribute {
                format: *format,
                offset: stride,
                shader_location: i as u32 + start_location,
            });

            stride += format.size() as u64;
        }

        VertexBufferLayout {
            array_stride: stride as u64,
            step_mode: mode,
            attributes,
        }
    }
}

impl From<Vec<MeshAttributeLayout>> for MeshLayout {
    fn from(attributes: Vec<MeshAttributeLayout>) -> Self {
        Self(attributes.into_boxed_slice())
    }
}

impl From<&[MeshAttributeLayout]> for MeshLayout {
    fn from(attributes: &[MeshAttributeLayout]) -> Self {
        Self(attributes.to_vec().into_boxed_slice())
    }
}

impl<A: AsRef<[MeshAttributeLayout]>> From<&A> for MeshLayout {
    fn from(attributes: &A) -> Self {
        Self(attributes.as_ref().to_vec().into_boxed_slice())
    }
}

impl std::ops::Deref for MeshLayout {
    type Target = [MeshAttributeLayout];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl IntoIterator for MeshLayout {
    type Item = MeshAttributeLayout;
    type IntoIter = std::vec::IntoIter<MeshAttributeLayout>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_vec().into_iter()
    }
}

impl<'a> IntoIterator for &'a MeshLayout {
    type Item = &'a MeshAttributeLayout;
    type IntoIter = std::slice::Iter<'a, MeshAttributeLayout>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum MeshFormat {
    Indexed { format: IndexFormat, count: u32 },
    NonIndexed,
}

pub struct RenderMesh {
    flags: MeshFlags,
    layout: MeshLayout,
    vertex_count: u32,
    format: MeshFormat,
    bounds: Aabb,
}

impl RenderMesh {
    pub fn flags(&self) -> MeshFlags {
        self.flags
    }

    pub fn layout(&self) -> &MeshLayout {
        &self.layout
    }

    pub fn vertex_count(&self) -> u32 {
        self.vertex_count
    }

    pub fn format(&self) -> MeshFormat {
        self.format
    }

    pub fn index_count(&self) -> Option<u32> {
        match self.format {
            MeshFormat::Indexed { count, .. } => Some(count),
            MeshFormat::NonIndexed => None,
        }
    }

    pub fn index_format(&self) -> Option<IndexFormat> {
        match self.format {
            MeshFormat::Indexed { format, .. } => Some(format),
            MeshFormat::NonIndexed => None,
        }
    }

    pub fn bounds(&self) -> &Aabb {
        &self.bounds
    }
}

impl From<&RenderMesh> for SubMesh {
    fn from(mesh: &RenderMesh) -> Self {
        Self {
            start_vertex: 0,
            vertex_count: mesh.vertex_count,
            start_index: 0,
            index_count: mesh.index_count().unwrap_or_default(),
        }
    }
}

impl RenderAsset for RenderMesh {
    type Source = Mesh;

    type Arg = ();

    fn extract(
        _: AssetId<Self::Source>,
        mut asset: Self::Source,
        _: &mut ArgItem<Self::Arg>,
    ) -> Result<Self, ExtractError<Self::Source>> {
        let mesh = asset.create_render_mesh();
        Ok(mesh)
    }
}

#[derive(Debug, Clone, Copy, Component)]
pub struct MeshFilter {
    pub mesh: AssetId<Mesh>,
    pub sub_mesh: Option<AssetId<SubMesh>>,
}

impl From<AssetId<Mesh>> for MeshFilter {
    fn from(mesh: AssetId<Mesh>) -> Self {
        Self {
            mesh,
            sub_mesh: None,
        }
    }
}
