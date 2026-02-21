use asset::{Asset, AssetId};
use render::{AsBinding, Shader};
use std::marker::PhantomData;
use transform::GlobalTransform;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum BlendMode {
    Opaque,
    Alpha,
    Additive,
    Multiplicative,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DepthWrite {
    Auto,
    Enable,
    Disable,
}

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct SortIndex(pub f32);

pub trait MaterialQueue: Send + Sync + Sized + 'static {
    const BLEND_MODE: BlendMode;

    const DEPTH_WRITE: DepthWrite;

    const SORT: bool = false;

    fn index(_entity: &GlobalTransform, _camera: &GlobalTransform) -> SortIndex {
        SortIndex(0.0)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct MaterialType<M: Material>(u32, PhantomData<M>);

pub trait Material: Asset + AsBinding {
    type Queue: MaterialQueue;

    fn shader() -> impl Into<AssetId<Shader>>;
}
