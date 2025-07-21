use crate::{
    IndexBuffer, Mesh, PipelineId, RenderAssets, RenderMesh, Shader, SubMesh, VertexBuffer,
    storage::{StorageBuffer, StorageBufferArray},
};
use asset::{AssetId, ErasedId};
use ecs::{Component, Entity, IndexMap};
use encase::{ShaderType, internal::WriteInto};
use math::{Mat4, bounds::Aabb};
use std::collections::HashMap;
use transform::{GlobalTransform, LocalTransform};
use wgpu::VertexFormat;

pub trait ViewData: ShaderType + WriteInto + Send + Sync + 'static {}

pub trait View: Component + Clone + 'static {
    type Data: ViewData;

    type Transform: LocalTransform;
}

pub trait Model: ShaderType + WriteInto + Send + Sync + 'static {}

impl<T: ShaderType + WriteInto + Send + Sync + 'static> Model for T {}

pub trait RenderPhase: Default + Copy + Ord + Eq + 'static {
    type View: View;

    const SORT: bool = false;

    /// Returns the a new render phase for the item
    /// with the given view and global transform.
    /// This is used to sort the items in the phase.
    fn new(
        view: &GlobalTransform,
        global: &GlobalTransform,
        local: &<Self::View as View>::Transform,
    ) -> Self;
}

pub trait Material: 'static {
    type View: View;

    type Phase: RenderPhase<View = Self::View>;
}

pub trait Drawable: Component + Clone + 'static {
    type View: View;

    type Model: Model;

    type Material: Material<View = Self::View>;

    const BATCH: bool = true;

    fn model(&self) -> Self::Model;

    fn state() -> wgpu::PrimitiveState {
        wgpu::PrimitiveState::default()
    }

    fn format() -> &'static [VertexFormat];

    fn shader() -> impl Into<AssetId<Shader>>;
}

pub struct DrawItem<D: Drawable> {
    pub entity: Entity,
    pub global: GlobalTransform,
    pub local: <D::View as View>::Transform,
    pub draw: D,
    pub key: BatchKey,
}

pub struct DrawSet<D: Drawable>(Vec<DrawItem<D>>);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BatchKey {
    material: ErasedId,
    mesh: AssetId<Mesh>,
    sub_mesh: AssetId<SubMesh>,
}

#[derive(Debug, Clone, Copy, ShaderType)]
pub struct IndirectDrawArgs {
    pub instance_count: u32,
    pub first_instance: u32,
    pub vertex_count: u32,
    pub first_vertex: u32,
}

#[derive(Debug, Clone, Copy, ShaderType)]
pub struct IndirectDrawIndexedArgs {
    pub index_count: u32,
    pub instance_count: u32,
    pub first_index: u32,
    pub base_vertex: i32,
    pub first_instance: u32,
}

pub struct RenderData {
    batch: u32,
    instance: u32,
    matrix: Mat4,
    bounds: Aabb,
}

pub struct DrawCall<P: RenderPhase> {
    index: usize,
    phase: P,
    pipeline: PipelineId,
}

pub struct ModelDataBuffer<M: Model>(StorageBufferArray<M>);

pub struct DrawArgBuffer(StorageBufferArray<IndirectDrawArgs>);
impl DrawArgBuffer {
    pub fn write(&mut self, args: &IndirectDrawArgs) -> u32 {
        todo!()
    }
}

pub struct DrawIndexedArgBuffer(StorageBufferArray<IndirectDrawIndexedArgs>);
impl DrawIndexedArgBuffer {
    pub fn write(&mut self, args: &IndirectDrawIndexedArgs) -> u32 {
        todo!()
    }
}

impl<M: Model> ModelDataBuffer<M> {
    pub fn write(&mut self, value: &M) -> u32 {
        self.0.push(value) / self.0.alignment()
    }
}

pub struct ViewDrawSets<P: RenderPhase>(HashMap<Entity, Vec<DrawCall<P>>>);

fn queue<D: Drawable>(
    drawables: &DrawSet<D>,
    buffer: &mut ModelDataBuffer<D::Model>,
    draw_buffer: &mut DrawArgBuffer,
    draw_indexed_buffer: &mut DrawIndexedArgBuffer,
    meshes: &RenderAssets<RenderMesh>,
) {
    let mut batches = IndexMap::new();

    for (index, drawable) in drawables.0.iter().enumerate() {
        batches
            .entry(drawable.key)
            .or_insert(Vec::new())
            .push(index);
    }

    for (batch, (key, indices)) in batches.iter().enumerate() {
        let dispatch_size = indices.len();
        for drawable in indices.iter().map(|i| &drawables.0[*i]) {
            // Write model data into global model data buffer
            let instance = buffer.write(&drawable.draw.model());
            let matrix = drawable.global.matrix();
            let bounds = *meshes.get(&key.mesh.into()).unwrap().bounds();
        }
    }
}
