use crate::asset::{AssetAction, AssetType, ErasedId};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoadState {
    Unloaded,
    Loading,
    Loaded,
    Failed,
}

impl LoadState {
    pub fn is_unloaded(self) -> bool {
        matches!(self, LoadState::Unloaded)
    }

    pub fn is_loading(self) -> bool {
        matches!(self, LoadState::Loading)
    }

    pub fn is_loaded(self) -> bool {
        matches!(self, LoadState::Loaded)
    }

    pub fn is_failed(self) -> bool {
        matches!(self, LoadState::Failed)
    }

    pub fn can_reload(self) -> bool {
        matches!(self, LoadState::Loaded | LoadState::Failed)
    }
}

#[derive(Debug)]
pub struct AssetState {
    ty: AssetType,
    state: LoadState,
    dependency_status: HashMap<ErasedId, LoadState>,
    dependencies: HashSet<ErasedId>,
    dependents: HashSet<ErasedId>,
    children: Vec<ErasedId>,
    parent: Option<ErasedId>,
    unload_action: Option<AssetAction>,
}

impl AssetState {
    pub fn new() -> Self {
        Self::with_state(LoadState::Unloaded)
    }

    pub fn with_state(state: LoadState) -> Self {
        Self {
            ty: AssetType::NONE,
            state,
            dependency_status: HashMap::new(),
            dependencies: HashSet::new(),
            dependents: HashSet::new(),
            children: vec![],
            parent: None,
            unload_action: None,
        }
    }

    pub fn ty(&self) -> AssetType {
        self.ty
    }

    pub fn state(&self) -> LoadState {
        self.state
    }

    pub fn dependency_status(&self) -> &HashMap<ErasedId, LoadState> {
        &self.dependency_status
    }

    pub fn dependencies(&self) -> &HashSet<ErasedId> {
        &self.dependencies
    }

    pub fn dependents(&self) -> &HashSet<ErasedId> {
        &self.dependents
    }

    pub fn parent(&self) -> Option<ErasedId> {
        self.parent
    }

    pub fn children(&self) -> &[ErasedId] {
        &self.children
    }

    pub fn unload_action(&self) -> Option<AssetAction> {
        self.unload_action
    }
}

#[derive(Debug, Default)]
pub struct AssetStates {
    states: HashMap<ErasedId, AssetState>,
}

impl AssetStates {
    pub fn new() -> Self {
        Self {
            states: HashMap::new(),
        }
    }

    pub fn get(&self, id: &ErasedId) -> Option<&AssetState> {
        self.states.get(id)
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

        for dependent in &state.dependents {
            let dependent = self.states.entry(*dependent).or_insert(AssetState::new());
            dependent.dependency_status.insert(id, LoadState::Loading);
        }

        self.states.insert(id, state);
    }

    pub fn loaded(
        &mut self,
        id: ErasedId,
        ty: AssetType,
        dependencies: &Vec<ErasedId>,
        parent: Option<ErasedId>,
        unload_action: Option<AssetAction>,
    ) -> Vec<(ErasedId, AssetType)> {
        let mut state = self.states.remove(&id).unwrap_or_else(AssetState::new);
        state.ty = ty;
        state.state = LoadState::Loaded;
        state.unload_action = unload_action;
        state.parent = parent;

        for dependency in dependencies.iter() {
            let dep_state = self.states.entry(*dependency).or_insert(AssetState::new());
            dep_state.dependents.insert(id);
            state.dependency_status.insert(*dependency, dep_state.state);
            state.dependencies.insert(*dependency);
        }

        if let Some(parent) = parent {
            let parent_state = self.states.entry(parent).or_insert(AssetState::new());
            parent_state.children.push(id);
            state.dependency_status.insert(parent, parent_state.state);
        }

        self.states.insert(id, state);

        match self.is_fully_loaded(&id) {
            true => self.finish(id),
            false => Vec::new(),
        }
    }

    pub fn unload(&mut self, id: ErasedId) -> Option<AssetState> {
        let mut state = self.states.remove(&id)?;
        state.state = LoadState::Unloaded;

        for dep in state.dependencies() {
            if let Some(state) = self.states.get_mut(dep) {
                state.dependents.remove(&id);
            }
        }

        for dep in state.dependents() {
            if let Some(state) = self.states.get_mut(dep) {
                state.dependency_status.remove(&id);
            }
        }

        Some(state)
    }

    pub fn failed(&mut self, id: ErasedId) {
        let mut state = self.states.remove(&id).unwrap_or_else(AssetState::new);
        state.state = LoadState::Failed;

        for dependent in state.dependents.iter().chain(state.children()) {
            let dependent = self.states.entry(*dependent).or_insert(AssetState::new());
            dependent.dependency_status.insert(id, LoadState::Failed);
        }

        self.states.insert(id, state);
    }

    pub fn is_fully_loaded(&self, id: &ErasedId) -> bool {
        let mut stack = vec![id];

        while let Some(id) = stack.pop() {
            let Some(state) = self.get(id) else {
                return false;
            };

            if !state.state.is_loaded() {
                return false;
            }

            stack.extend(state.dependency_status().keys());
        }

        return true;
    }

    fn finish(&mut self, id: ErasedId) -> Vec<(ErasedId, AssetType)> {
        let mut visited = HashSet::new();
        let mut stack = vec![id];
        let mut finished = vec![];

        while let Some(id) = stack.pop() {
            if visited.contains(&id) {
                continue;
            }

            visited.insert(id);

            let (ty, dependents) = match self.states.get(&id) {
                Some(state) => {
                    let dependents = state
                        .dependents
                        .iter()
                        .chain(state.children())
                        .copied()
                        .collect::<Vec<_>>();

                    (state.ty, dependents)
                }
                None => continue,
            };

            for dependent in dependents {
                let Some(state) = self.states.get_mut(&dependent) else {
                    continue;
                };

                state.dependency_status.remove(&id);

                if state.dependency_status.is_empty() {
                    stack.push(dependent);
                }
            }

            finished.push((id, ty));
        }

        finished
    }
}
