use super::{
    VertexBufferLayout,
    buffer::{IndexBuffer, Indices, VertexBuffer},
    extract::{ExtractError, ReadWrite, RenderAsset, RenderAssetExtractor},
};
use crate::{RenderAssetType, device::RenderDevice, types::Color};
use asset::{Asset, Settings, importer::AssetImporter};
use derive_render::RenderAsset;
use ecs::system::{ArgItem, unlifetime::Read};
use math::bounds::Bounds;
use smol::io::AsyncAsSync;
use std::{hash::Hash, ops::Range};
use waker_fn::waker_fn;
use wgpu::{BufferUsages, VertexStepMode};

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

#[derive(
    Debug,
    Clone,
    Copy,
    Eq,
    PartialEq,
    Hash,
    Asset,
    RenderAsset,
    serde::Serialize,
    serde::Deserialize,
)]
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

#[derive(Clone, Asset, serde::Serialize, serde::Deserialize)]
pub struct Mesh {
    topology: MeshTopology,
    attributes: Vec<MeshAttribute>,
    indices: Option<Indices>,
    bounds: Bounds,
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
            bounds: Bounds::ZERO,
            read_write: ReadWrite::Disabled,
            dirty: MeshDirty::empty(),
        }
    }

    pub fn with_attribute(mut self, attribute: MeshAttribute) -> Self {
        self.add_attribute(attribute);
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

    pub fn bounds(&self) -> Bounds {
        self.bounds
    }

    pub fn read_write(&self) -> ReadWrite {
        self.read_write
    }

    pub fn add_attribute(&mut self, attribute: MeshAttribute) {
        let ty = attribute.ty;
        match self.attribute_index(ty) {
            Some(i) => self.attributes[i] = attribute,
            None => self.attributes.push(attribute),
        }

        self.attribute_dirty(ty);
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
                self.bounds = Bounds::from(positions.as_slice());
                self.dirty.remove(MeshDirty::BOUNDS);
            }
            (true, MeshAttributeValues::Vec2(positions)) => {
                self.bounds = Bounds::from(positions.as_slice());
                self.dirty.remove(MeshDirty::BOUNDS);
            }
            (true, MeshAttributeValues::Vec4(positions)) => {
                self.bounds = Bounds::from(positions.as_slice());
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

    pub fn create_render_mesh(&mut self, device: &RenderDevice) -> RenderMesh {
        let (data, _) = self.vertex_data();

        let mut layout = vec![];
        let mut stride = 0;
        for attribute in &mut self.attributes {
            stride += attribute.values.size();

            layout.push(MeshAttributeLayout {
                ty: attribute.ty,
                format: attribute.values.format(),
            });

            if self.read_write == ReadWrite::Disabled {
                attribute.values.clear();
            }
        }

        let usage = match self.read_write {
            ReadWrite::Enabled => BufferUsages::COPY_DST | BufferUsages::MAP_WRITE,
            ReadWrite::Disabled => BufferUsages::empty(),
        };

        let vertex_buffer = VertexBuffer::new_from_data(device, &data, stride, Some(usage));

        let index_buffer = self.indices.as_mut().map(|indices| {
            let buffer = IndexBuffer::new(device, &indices, Some(usage));
            if self.read_write == ReadWrite::Disabled {
                indices.clear();
            }
            buffer
        });

        RenderMesh {
            layout: layout.into(),
            vertex_buffer,
            index_buffer,
        }
    }

    pub fn update(&mut self, device: &RenderDevice, buffers: &mut RenderMesh) {
        let (data, _) = self.vertex_data();
        let stride = self
            .attributes
            .iter()
            .fold(0, |sum, a| sum + a.values.size());

        buffers.vertex_buffer = VertexBuffer::new_from_data(device, &data, stride, None);

        if self.dirty.contains(MeshDirty::INDICES) {
            match (buffers.index_buffer.as_mut(), self.indices()) {
                (Some(index), Some(indices)) => {
                    index.update(device, indices);
                    self.dirty.remove(MeshDirty::INDICES);
                }
                _ => (),
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MeshAttributeLayout {
    pub ty: MeshAttributeType,
    pub format: wgpu::VertexFormat,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeshLayout(Box<[MeshAttributeLayout]>);

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

pub struct RenderMesh {
    layout: MeshLayout,
    vertex_buffer: VertexBuffer,
    index_buffer: Option<IndexBuffer>,
}

impl RenderMesh {
    pub fn layout(&self) -> &MeshLayout {
        &self.layout
    }

    pub fn vertex_buffer(&self) -> &VertexBuffer {
        &self.vertex_buffer
    }

    pub fn index_buffer(&self) -> Option<&IndexBuffer> {
        self.index_buffer.as_ref()
    }

    pub fn vertex_count(&self) -> usize {
        self.vertex_buffer.len()
    }

    pub fn index_count(&self) -> usize {
        self.index_buffer.as_ref().map_or(0, |i| i.len())
    }
}

impl From<&RenderMesh> for SubMesh {
    fn from(mesh: &RenderMesh) -> Self {
        SubMesh::new(0, mesh.vertex_count() as u32, 0, mesh.index_count() as u32)
    }
}

impl RenderAsset for RenderMesh {}

impl RenderAssetExtractor for Mesh {
    type RenderAsset = RenderMesh;

    type Arg = Read<RenderDevice>;

    fn extract(
        mut asset: Self,
        arg: &mut ArgItem<Self::Arg>,
    ) -> Result<Self::RenderAsset, ExtractError<Self>> {
        let mesh = asset.create_render_mesh(arg);
        Ok(mesh)
    }
}

#[derive(Default, Settings, serde::Serialize, serde::Deserialize)]
pub struct MeshImportSettings {
    read_write: ReadWrite,
}

pub struct ObjImporter;

impl AssetImporter for ObjImporter {
    type Asset = Mesh;

    type Settings = MeshImportSettings;

    type Error = tobj::LoadError;

    async fn import(
        ctx: &mut asset::importer::ImportContext<'_>,
        reader: &mut dyn asset::io::AsyncReader,
        metadata: &asset::AssetMetadata<Self::Settings>,
    ) -> Result<Self::Asset, Self::Error> {
        let waker = waker_fn(|| {});
        let mut context = std::task::Context::from_waker(&waker);
        let reader = AsyncAsSync::new(&mut context, reader);
        let mut reader = std::io::BufReader::new(reader);

        let (models, _) = tobj::load_obj_buf(&mut reader, &tobj::LoadOptions::default(), |_| {
            Ok((vec![], Default::default()))
        })?;

        let mut positions = Vec::new();
        let mut tex_coords = Vec::new();
        let mut normals = Vec::new();
        let mut indices = Vec::new();

        for model in models {
            let start_vertex = positions.len();
            let start_index = indices.len();

            for vertices in model.mesh.positions.chunks(3) {
                positions.push(math::Vec3::from_slice(vertices));
            }

            for tex_coord in model.mesh.texcoord_indices.chunks(2) {
                let coord = math::Vec2::new(
                    model.mesh.texcoords[tex_coord[0] as usize],
                    model.mesh.texcoords[tex_coord[1] as usize],
                );
                tex_coords.push(coord);
            }

            for normal in model.mesh.normal_indices.chunks(3) {
                let normal = math::Vec3::new(
                    model.mesh.normals[normal[0] as usize],
                    model.mesh.normals[normal[1] as usize],
                    model.mesh.normals[normal[2] as usize],
                );
                normals.push(normal);
            }

            indices.extend(model.mesh.indices);

            let vertex_count = positions.len() - start_vertex;
            let index_count = indices.len() - start_index;

            let _ = ctx.add_child(
                model.name,
                SubMesh {
                    start_vertex: start_vertex as u32,
                    vertex_count: vertex_count as u32,
                    start_index: start_index as u32,
                    index_count: index_count as u32,
                },
            );
        }

        let mut mesh = Mesh::new(MeshTopology::TriangleList);
        if !positions.is_empty() {
            mesh.add_attribute(MeshAttribute {
                ty: MeshAttributeType::Position,
                values: MeshAttributeValues::Vec3(positions),
            });
        }

        if !tex_coords.is_empty() {
            mesh.add_attribute(MeshAttribute {
                ty: MeshAttributeType::TexCoord0,
                values: MeshAttributeValues::Vec2(tex_coords),
            });
        }

        if !normals.is_empty() {
            mesh.add_attribute(MeshAttribute {
                ty: MeshAttributeType::Normal,
                values: MeshAttributeValues::Vec3(normals),
            });
        }

        if !indices.is_empty() {
            mesh.set_indices(Indices::new::<u32>(&indices));
        }

        mesh.read_write = metadata.read_write;

        Ok(mesh)
    }
}

impl RenderAssetExtractor for SubMesh {
    type RenderAsset = SubMesh;

    type Arg = ();

    fn extract(
        asset: Self,
        _: &mut ArgItem<Self::Arg>,
    ) -> Result<Self::RenderAsset, ExtractError<Self>> {
        Ok(asset)
    }

    fn dependencies() -> Vec<super::RenderAssetType> {
        vec![RenderAssetType::of::<RenderMesh>()]
    }
}
