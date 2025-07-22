use crate::{
    ArrayBuffer, IndexBuffer, Mesh, MeshFormat, PipelineId, RenderAssets, RenderMesh, Shader,
    SubMesh, VertexBuffer,
    storage::{StorageBuffer, StorageBufferArray},
    uniform::UniformBufferArray,
};
use asset::{AssetId, ErasedId};
use ecs::{Component, Entity, IndexMap};
use encase::{ShaderType, internal::WriteInto};
use math::{Mat4, Vec3, bounds::Aabb};
use std::{collections::HashMap, ops::Range};
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

#[derive(Default, Debug, Clone, Copy, ShaderType, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct IndirectDrawArgs {
    pub instance_count: u32,
    pub first_instance: u32,
    pub vertex_count: u32,
    pub first_vertex: u32,
}

#[derive(Default, Debug, Clone, Copy, ShaderType, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct IndirectDrawIndexedArgs {
    pub index_count: u32,
    pub instance_count: u32,
    pub first_index: u32,
    pub base_vertex: i32,
    pub first_instance: u32,
}

#[derive(Clone, Copy, ShaderType)]
pub struct RenderData {
    batch: u32,
    instance: u32,
    matrix: Mat4,
    min_bounds: Vec3,
    max_bounds: Vec3,
    indexed: u32,
}

pub struct ViewDataBuffer<V: View> {
    buffer: UniformBufferArray<V::Data>,
    views: HashMap<Entity, V>,
}

pub struct RenderDataBuffer(StorageBufferArray<RenderData>);
impl RenderDataBuffer {
    pub fn write(&mut self, data: &RenderData) -> u32 {
        todo!()
    }
}

pub struct ModelDataBuffer<M: Model>(StorageBufferArray<M>);

pub struct DrawArgBuffers {
    non_indexed: ArrayBuffer<IndirectDrawArgs>,

    indexed: ArrayBuffer<IndirectDrawIndexedArgs>,
}

impl DrawArgBuffers {
    pub fn push(&mut self, args: IndirectDrawArgs) -> u32 {
        self.non_indexed.push(args)
    }

    pub fn push_indexed(&mut self, args: IndirectDrawIndexedArgs) -> u32 {
        self.indexed.push(args)
    }
}

impl<M: Model> ModelDataBuffer<M> {
    pub fn write(&mut self, value: &M) -> u32 {
        self.0.push(value) / self.0.alignment()
    }
}

pub struct ViewDrawCalls<P: RenderPhase>(HashMap<Entity, Vec<DrawCall<P>>>);

#[derive(Clone)]
pub enum DrawMode {
    Direct {
        dynamic_offset: u32,
        bind_group: u32,
        instances: Range<u32>,
    },
    Indirect,
}

#[derive(Clone)]
pub struct DrawCall<P: RenderPhase> {
    key: BatchKey,
    phase: P,
    pipeline: PipelineId,
    mode: DrawMode,
    function: u32,
}

fn queue<D: Drawable>(
    drawables: &DrawSet<D>,
    draw_calls: &mut ViewDrawCalls<<D::Material as Material>::Phase>,
    render_buffer: &mut RenderDataBuffer,
    model_buffer: &mut ModelDataBuffer<D::Model>,
    draw_arg_buffers: &mut DrawArgBuffers,
    views: &ViewDataBuffer<D::View>,
    meshes: &RenderAssets<RenderMesh>,
) {
    let mut batches = HashMap::new();

    for (index, drawable) in drawables.0.iter().enumerate() {
        batches
            .entry(drawable.key)
            .or_insert(Vec::new())
            .push(index);
    }

    for (key, items) in batches {
        let Some(mesh) = meshes.get(&key.mesh) else {
            continue;
        };

        let (batch, bounds, indexed) = match mesh.format() {
            MeshFormat::Indexed { count, .. } => {
                let args = IndirectDrawIndexedArgs {
                    index_count: count,
                    ..Default::default()
                };
                let index = draw_arg_buffers.push_indexed(args);
                (index, *mesh.bounds(), 1)
            }
            MeshFormat::NonIndexed => {
                let args = IndirectDrawArgs {
                    vertex_count: mesh.vertex_count(),
                    ..Default::default()
                };

                let index = draw_arg_buffers.push(args);
                (index, *mesh.bounds(), 0)
            }
        };

        for drawable in items.iter().map(|i| &drawables.0[*i]) {
            let instance = model_buffer.write(&drawable.draw.model()) / model_buffer.0.alignment();
            render_buffer.write(&RenderData {
                batch,
                instance,
                matrix: drawable.global.matrix(),
                min_bounds: bounds.min,
                max_bounds: bounds.max,
                indexed,
            });
        }

        let draw = DrawCall {
            key,
            phase: <D::Material as Material>::Phase::default(),
            pipeline: todo!(),
            mode: DrawMode::Indirect,
            function: todo!(),
        };

        for view in views.views.keys() {
            draw_calls.0.entry(*view).or_default().push(draw.clone());
        }
    }

    // wgpu::RenderPass::draw_indirect(&mut self, draw_arg_buffers.non_indexed.buffer(), 0);
    // wgpu::ComputePass::dispatch_workgroups(&mut self, size as u32, y, z);
}
