use crate::{SparseIndex, ext};
use std::{alloc::Layout, any::TypeId, collections::HashMap};

pub trait Component: Send + Sync + 'static {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ComponentId(pub(crate) u32);

pub struct ComponentMeta {
    id: ComponentId,
    name: &'static str,
    layout: Layout,
}

impl ComponentMeta {
    pub fn new<C: Component>(id: ComponentId) -> Self {
        Self {
            id,
            name: ext::short_type_name::<C>(),
            layout: Layout::new::<C>(),
        }
    }

    pub fn id(&self) -> ComponentId {
        self.id
    }

    pub fn name(&self) -> &'static str {
        self.name
    }

    pub fn layout(&self) -> Layout {
        self.layout
    }
}

pub struct Components {
    components: Vec<ComponentMeta>,
    map: HashMap<TypeId, ComponentId>,
}

impl Components {
    pub fn new() -> Self {
        Self {
            components: vec![],
            map: HashMap::new(),
        }
    }

    pub fn register<C: Component>(&mut self) -> ComponentId {
        let ty = TypeId::of::<C>();
        match self.map.get(&ty) {
            Some(id) => *id,
            None => {
                let id = ComponentId(self.components.len() as u32);
                let meta = ComponentMeta::new::<C>(id);

                self.components.push(meta);
                self.map.insert(ty, id);

                id
            }
        }
    }

    pub fn get<C: Component>(&self) -> Option<&ComponentMeta> {
        self.map
            .get(&TypeId::of::<C>())
            .and_then(|id| self.components.get(id.0 as usize))
    }

    pub fn get_by_id(&self, id: ComponentId) -> Option<&ComponentMeta> {
        self.components.get(id.to_usize())
    }

    pub fn get_id<C: Component>(&self) -> Option<ComponentId> {
        self.map.get(&TypeId::of::<C>()).copied()
    }

    pub unsafe fn get_id_unchecked<C: Component>(&self) -> ComponentId {
        self.map
            .get(&TypeId::of::<C>())
            .copied()
            .unwrap_or_else(|| panic!("Component not registered: {}", std::any::type_name::<C>()))
    }

    pub fn metas(&self) -> &[ComponentMeta] {
        &self.components
    }

    pub fn len(&self) -> usize {
        self.components.len()
    }
}
