use crate::{Mesh, PipelineId, Shader, SubMesh};
use asset::{AssetId, ErasedId};
use ecs::{Component, Entity};
use encase::{ShaderType, internal::WriteInto};
use math::{Mat4, bounds::Aabb};
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

    /// Returns the sort key for the given view.
    fn sort_key(view: &Self::View) -> u64;

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

pub trait Draw: Component + Clone + 'static {
    type View: View;

    type Model: Model;

    type Material: Material<View = Self::View>;

    const BATCH: bool = true;

    fn state() -> wgpu::PrimitiveState {
        wgpu::PrimitiveState::default()
    }

    fn format() -> &'static [VertexFormat];

    fn shader() -> impl Into<AssetId<Shader>>;
}

pub struct DrawItem<D: Draw> {
    pub entity: Entity,
    pub draw: D,
    pub global: GlobalTransform,
    pub local: <D::View as View>::Transform,
}

pub struct DrawSet<D: Draw>(Vec<DrawItem<D>>);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BatchKey {
    material: ErasedId,
    mesh: AssetId<Mesh>,
    sub_mesh: AssetId<SubMesh>,
}

pub struct CullData {
    index: u32,
    matrix: Mat4,
    bounds: Aabb,
}

pub struct DrawCall<P: RenderPhase> {
    index: usize,
    phase: P,
    pipeline: PipelineId,
}

pub struct ViewDrawSets<P: RenderPhase>(HashMap<Entity, Vec<DrawCall<P>>>);
