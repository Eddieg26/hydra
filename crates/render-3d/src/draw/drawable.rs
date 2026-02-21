use crate::material::{Material, MaterialQueue};
use asset::{AssetId, ErasedId};
use ecs::{Component, Resource};
use render::{
    Mesh, MeshFlags, Shader, ShaderType, SubMesh,
    storage::StorageBufferArray,
    wgpu::{DynamicOffset, PrimitiveState},
};
use std::marker::PhantomData;

pub trait Drawable: Component {
    type Material: Material;

    fn material(&self) -> AssetId<Self::Material>;

    fn flags() -> MeshFlags;

    fn state() -> PrimitiveState {
        PrimitiveState::default()
    }

    fn shader() -> impl Into<AssetId<Shader>>;
}

#[derive(Clone, Copy, ShaderType)]
pub struct DrawArgs {
    /// The number of indices to draw.
    pub index_count: u32,
    /// The number of instances to draw.
    pub instance_count: u32,
    /// The first index within the index buffer.
    pub first_index: u32,
    /// The value added to the vertex index before indexing into the vertex buffer.
    pub base_vertex: i32,
    /// The instance ID of the first instance to draw.
    ///
    /// Has to be 0, unless [`Features::INDIRECT_FIRST_INSTANCE`](crate::Features::INDIRECT_FIRST_INSTANCE) is enabled.
    pub first_instance: u32,
}

#[derive(Resource)]
pub struct DrawArgBuffer(StorageBufferArray<DrawArgs>);

impl std::ops::Deref for DrawArgBuffer {
    type Target = StorageBufferArray<DrawArgs>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for DrawArgBuffer {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

pub struct DrawCall {
    material: ErasedId,
    mesh: AssetId<Mesh>,
    sub_mesh: AssetId<SubMesh>,
    offset: DynamicOffset,
}

pub struct DrawCalls<M: MaterialQueue>(Vec<DrawCall>, PhantomData<M>);

impl<M: MaterialQueue> DrawCalls<M> {
    pub fn push(&mut self, call: DrawCall) {
        self.0.push(call);
    }
}
