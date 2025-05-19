use crate::asset::{AssetType, ErasedId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoadState {
    Unloaded,
    Loading,
    Loaded,
    Failed,
}

pub struct AssetState {
    ty: AssetType,
    state: LoadState,
    dependency_state: LoadState,
    dependencies: Vec<ErasedId>,
    dependents: Vec<ErasedId>,
}
