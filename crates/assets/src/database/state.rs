use crate::asset::{AssetType, ErasedId};
use std::collections::{HashMap, HashSet};

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
    dependencies: HashSet<ErasedId>,
    dependents: HashSet<ErasedId>,
    loading: HashSet<ErasedId>,
    failed: HashSet<ErasedId>,
}

impl AssetState {
    pub fn new() -> Self {
        Self::with_state(LoadState::Unloaded)
    }

    pub fn with_state(state: LoadState) -> Self {
        Self {
            ty: AssetType::NONE,
            state,
            dependency_state: LoadState::Unloaded,
            dependencies: HashSet::new(),
            dependents: HashSet::new(),
            loading: HashSet::new(),
            failed: HashSet::new(),
        }
    }

    pub fn state(&self) -> LoadState {
        self.state
    }

    pub fn dependency_state(&self) -> LoadState {
        self.dependency_state
    }

    pub fn dependencies(&self) -> &HashSet<ErasedId> {
        &self.dependencies
    }

    pub fn dependents(&self) -> &HashSet<ErasedId> {
        &self.dependents
    }

    pub fn loading(&self) -> &HashSet<ErasedId> {
        &self.loading
    }

    pub fn failed(&self) -> &HashSet<ErasedId> {
        &self.failed
    }

    pub fn is_fully_loaded(&self) -> bool {
        matches!(
            (self.state, self.dependency_state),
            (LoadState::Loaded, LoadState::Loaded | LoadState::Failed)
        )
    }

    pub fn update(&mut self) {
        self.dependency_state = if self.loading.len() > 0 {
            LoadState::Loading
        } else if self.failed.len() > 0 {
            LoadState::Failed
        } else {
            LoadState::Loaded
        };
    }
}

#[derive(Default)]
pub struct AssetStates {
    states: HashMap<ErasedId, AssetState>,
}

impl AssetStates {
    pub fn new() -> Self {
        Self {
            states: HashMap::new(),
        }
    }

    pub fn get(&self, id: ErasedId) -> Option<&AssetState> {
        self.states.get(&id)
    }

    pub fn get_load_state(&self, id: ErasedId) -> LoadState {
        self.states
            .get(&id)
            .map(|s| s.state)
            .unwrap_or(LoadState::Unloaded)
    }

    pub fn loading(&mut self, id: ErasedId) {
        let mut state = self.states.remove(&id).unwrap_or_else(AssetState::new);

        state.state = LoadState::Loading;

        for dep in state.dependents.iter() {
            if let Some(state) = self.states.get_mut(dep) {
                state.loading.insert(id);
                state.failed.remove(&id);
                state.dependency_state = LoadState::Loading;
            }
        }

        self.states.insert(id, state);
    }

    pub fn loaded(
        &mut self,
        id: ErasedId,
        ty: AssetType,
        dependencies: Vec<ErasedId>,
    ) -> HashSet<ErasedId> {
        let mut state = self.states.remove(&id).unwrap_or_else(AssetState::new);

        state.ty = ty;
        state.state = LoadState::Loaded;

        for dependency in &dependencies {
            match self.states.get_mut(dependency) {
                Some(dep_state) => {
                    dep_state.dependents.insert(id);
                    match dep_state.state {
                        LoadState::Loading => state.loading.insert(*dependency),
                        LoadState::Failed => state.failed.insert(*dependency),
                        _ => continue,
                    };
                }
                None => {
                    let mut dep_state = AssetState::new();
                    dep_state.dependents.insert(id);
                    state.loading.insert(*dependency);
                    self.states.insert(*dependency, dep_state);
                }
            };
        }

        state.update();

        for dependent in &state.dependents {
            if let Some(state) = self.states.get_mut(dependent) {
                state.loading.remove(&id);
                state.failed.remove(&id);
                state.update();
            }
        }

        self.states.insert(id, state);

        self.finish(id)
    }

    pub fn unload(&mut self, id: ErasedId) -> Option<AssetState> {
        let mut state = self.states.remove(&id)?;
        state.state = LoadState::Unloaded;

        for dep in &state.dependencies {
            if let Some(state) = self.states.get_mut(dep) {
                state.dependents.remove(&id);
            }
        }

        for dep in &state.dependents {
            if let Some(state) = self.states.get_mut(dep) {
                state.dependencies.remove(&id);
            }
        }

        Some(state)
    }

    pub fn failed(&mut self, id: ErasedId) -> HashSet<ErasedId> {
        let mut state = self.states.remove(&id).unwrap_or_else(AssetState::new);
        state.state = LoadState::Failed;

        for dep in &state.dependents {
            if let Some(state) = self.states.get_mut(dep) {
                state.loading.remove(&id);
                state.failed.insert(id);
                state.update();
            }
        }

        self.states.insert(id, state);

        self.finish(id)
    }

    fn finish(&mut self, id: ErasedId) -> HashSet<ErasedId> {
        let mut finished = HashSet::new();
        let mut visited = HashSet::new();
        let mut stack = vec![id];

        while let Some(id) = stack.pop() {
            if visited.contains(&id) {
                continue;
            }

            visited.insert(id);

            let state = match self.states.get(&id) {
                Some(state) => state,
                None => continue,
            };

            if state.dependencies.iter().all(|dep| finished.contains(dep)) {
                finished.insert(id);
            }

            stack.extend(&state.dependents);
        }

        finished
    }
}
