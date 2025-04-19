use super::{Component, ComponentId, Components, Entity};
use crate::core::{Frame, bitset::FixedBitSet, sparse::SparseIndex};
use std::{collections::HashMap, fmt::Debug};

pub mod table;

pub use table::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ArchetypeId(pub u32);

impl ArchetypeId {
    pub const EMPTY: Self = Self(0);
}

pub struct Archetype {
    id: ArchetypeId,
    table: Table,
    bitset: FixedBitSet,
}

impl Archetype {
    pub fn new(id: ArchetypeId, table: Table, bitset: FixedBitSet) -> Self {
        Self { id, table, bitset }
    }

    pub fn id(&self) -> ArchetypeId {
        self.id
    }

    pub fn table(&self) -> &Table {
        &self.table
    }

    pub fn table_mut(&mut self) -> &mut Table {
        &mut self.table
    }

    pub fn contains(&self, entity: Entity) -> bool {
        self.table.contains(entity)
    }

    pub fn matches(&self, query: &ArchetypeQuery) -> bool {
        self.bitset.is_superset(&query.include)
            && self.bitset.is_disjoint(&query.exclude)
            && (query.any.is_clear() || !self.bitset.is_disjoint(&query.any))
    }

    pub fn has_components(&self, components: &FixedBitSet) -> bool {
        self.bitset.is_superset(components)
    }

    pub fn has_component(&self, component: usize) -> bool {
        self.bitset[component]
    }

    pub fn has_component_id(&self, id: ComponentId) -> bool {
        self.table.has_component(id)
    }

    pub fn add_entity(&mut self, entity: Entity, row: Row) {
        self.table.add_entity(entity, row);
    }

    pub fn remove_entity(&mut self, entity: Entity) -> Option<Row> {
        self.table.remove_entity(entity)
    }

    pub fn modify_component(&mut self, entity: Entity, id: ComponentId, frame: Frame) {
        self.table.modify_component(entity, id, frame);
    }
}

pub struct Archetypes {
    archetypes: Vec<Archetype>,
    archetype_map: HashMap<Box<[ComponentId]>, ArchetypeId>,
    entity_map: HashMap<Entity, ArchetypeId>,
    components: Components,
    bitset: FixedBitSet,
}

impl Archetypes {
    pub fn new() -> Self {
        let archetypes = vec![Archetype::new(
            ArchetypeId::EMPTY,
            TableBuilder::new().build(),
            FixedBitSet::new(),
        )];

        let mut archetype_map: HashMap<Box<[ComponentId]>, ArchetypeId> = HashMap::new();
        archetype_map.insert(Box::new([]), ArchetypeId::EMPTY);

        Self {
            archetypes,
            archetype_map,
            entity_map: HashMap::new(),
            components: Components::new(),
            bitset: FixedBitSet::new(),
        }
    }

    pub fn register<C: Component>(&mut self) -> ComponentId {
        let id = self.components.register::<C>();
        self.bitset.grow(id.to_usize() + 1);
        id
    }

    pub fn archetypes(&self) -> &Vec<Archetype> {
        &self.archetypes
    }

    pub fn archetype(&self, id: ArchetypeId) -> Option<&Archetype> {
        self.archetypes.get(id.0 as usize)
    }

    pub fn entity_archetype(&self, entity: Entity) -> Option<ArchetypeId> {
        self.entity_map.get(&entity).copied()
    }

    pub fn components(&self) -> &Components {
        &self.components
    }

    pub fn components_mut(&mut self) -> &mut Components {
        &mut self.components
    }

    pub fn query(&self, query: &ArchetypeQuery) -> Vec<&Archetype> {
        let mut archetypes = Vec::new();
        for archetype in &self.archetypes {
            if archetype.matches(query) {
                archetypes.push(archetype);
            }
        }

        archetypes
    }

    pub fn add_entity(&mut self, entity: Entity) -> ArchetypeId {
        match self.entity_map.get(&entity).copied() {
            Some(id) => id,
            None => {
                let archetype_id = ArchetypeId::EMPTY;
                self.entity_map.insert(entity, archetype_id);
                self.archetypes[archetype_id.0 as usize]
                    .table
                    .add_entity(entity, Row::new());
                archetype_id
            }
        }
    }

    pub fn remove_entity(&mut self, entity: Entity) -> Option<(ArchetypeId, Row)> {
        let id = self.entity_map.remove(&entity)?;
        let archetype = &mut self.archetypes[id.0 as usize];
        let row = archetype.remove_entity(entity)?;

        Some((id, row))
    }

    pub fn get_component<C: Component>(&self, entity: Entity) -> Option<&C> {
        let id = unsafe { self.components.get_id_unchecked::<C>() };
        let archetype_id = self.entity_map.get(&entity)?;
        let archetype = &self.archetypes[archetype_id.0 as usize];
        archetype.table.get_component(entity, id)
    }

    pub fn get_component_mut<C: Component>(&mut self, entity: Entity) -> Option<&mut C> {
        let id = unsafe { self.components.get_id_unchecked::<C>() };
        let archetype_id = self.entity_map.get(&entity)?;
        let archetype = &mut self.archetypes[archetype_id.0 as usize];
        archetype.table.get_component_mut(entity, id)
    }

    pub fn add_component<C: Component>(&mut self, entity: Entity, component: C, frame: Frame) {
        let id = unsafe { self.components.get_id_unchecked::<C>() };

        let (_, mut row) = match self.remove_entity(entity) {
            Some((id, row)) => (id, row),
            None => (ArchetypeId::EMPTY, Row::new()),
        };

        let mut component = TableCell::new(component);
        match row.contains(id) {
            true => component.modify(frame),
            false => component.add(frame),
        }

        row.insert_cell(id, component);

        self.add_entity_inner(entity, row);
    }

    pub fn add_components(&mut self, entity: Entity, mut components: Row, frame: Frame) {
        let (_, mut row) = match self.remove_entity(entity) {
            Some((id, row)) => (id, row),
            None => (ArchetypeId::EMPTY, Row::new()),
        };

        while let Some((id, mut component)) = components.remove_at(0) {
            match row.contains(id) {
                true => component.modify(frame),
                false => component.add(frame),
            }

            row.insert_cell(id, component);
        }

        self.add_entity_inner(entity, row);
    }

    pub fn remove_component<C: Component>(&mut self, entity: Entity) -> Option<C> {
        let id = unsafe { self.components.get_id_unchecked::<C>() };

        let (_, mut row) = match self.remove_entity(entity) {
            Some(value) => value,
            None => return None,
        };

        let component = row.remove(id);

        self.add_entity_inner(entity, row);

        component.map(|c| c.into_value())
    }

    pub fn remove_components(
        &mut self,
        entity: Entity,
        components: Vec<ComponentId>,
    ) -> Option<Row> {
        let (_, mut row) = match self.remove_entity(entity) {
            Some((id, row)) => (id, row),
            None => return None,
        };

        let mut removed = Row::new();
        for id in components {
            if let Some(value) = row.remove(id) {
                removed.insert_cell(id, value);
            }
        }

        self.add_entity_inner(entity, row);

        Some(removed)
    }

    pub fn modify_component<C: Component>(&mut self, entity: Entity, frame: Frame) {
        let id = unsafe { self.components.get_id_unchecked::<C>() };

        let Some(archetype_id) = self.entity_map.get(&entity) else {
            return;
        };
        let archetype = &mut self.archetypes[archetype_id.0 as usize];
        archetype.modify_component(entity, id, frame);
    }

    #[inline]
    fn add_entity_inner(&mut self, entity: Entity, components: Row) -> ArchetypeId {
        let mut ids = components.ids().to_vec();
        ids.sort();

        let id = ids.into_boxed_slice();

        match self.archetype_map.get(&id).copied() {
            Some(id) => {
                let archetype = &mut self.archetypes[id.0 as usize];
                archetype.table.add_entity(entity, components);
                self.entity_map.insert(entity, id);

                id
            }
            None => {
                let mut bits = self.bitset.clone();
                id.iter().for_each(|id| bits.set(id.to_usize(), true));

                if id.len() > 1 {
                    println!("Archetype with multiple components: {:?}", id);
                }

                let archetype_id = ArchetypeId(self.archetypes.len() as u32);
                let archetype = Archetype::new(archetype_id, components.into_table(entity), bits);

                self.archetypes.push(archetype);
                self.entity_map.insert(entity, archetype_id);
                self.archetype_map.insert(id, archetype_id);
                archetype_id
            }
        }
    }
}

impl std::ops::Index<ArchetypeId> for Archetypes {
    type Output = Archetype;

    fn index(&self, index: ArchetypeId) -> &Self::Output {
        &self.archetypes[index.0 as usize]
    }
}

impl std::ops::IndexMut<ArchetypeId> for Archetypes {
    fn index_mut(&mut self, index: ArchetypeId) -> &mut Self::Output {
        &mut self.archetypes[index.0 as usize]
    }
}

#[derive(Debug, Clone, Default)]
pub struct ArchetypeQuery {
    pub include: FixedBitSet,
    pub exclude: FixedBitSet,
    pub any: FixedBitSet,
}

impl ArchetypeQuery {
    pub fn include(&mut self, id: ComponentId) -> ComponentId {
        self.include.grow(id.to_usize() + 1);
        self.include.set(id.to_usize(), true);
        id
    }

    pub fn exclude(&mut self, id: ComponentId) -> ComponentId {
        self.exclude.grow(id.to_usize() + 1);
        self.exclude.set(id.to_usize(), true);
        id
    }

    pub fn any(&mut self, id: ComponentId) -> ComponentId {
        self.any.grow(id.to_usize() + 1);
        self.any.set(id.to_usize(), true);
        id
    }
}

mod tests {
    use crate::{
        core::Frame,
        world::{Component, Entity, Row},
    };

    use super::{ArchetypeQuery, Archetypes};

    #[derive(Debug, PartialEq, Eq)]
    struct Age(u32);
    impl Component for Age {}

    #[derive(Debug, PartialEq, Eq)]
    struct Name(&'static str);
    impl Component for Name {}

    #[test]
    fn archetype_add_entity() {
        let mut archetypes = Archetypes::new();
        let entity = Entity::root(0);
        let archetype = archetypes.add_entity(entity);

        assert_eq!(archetypes.entity_archetype(entity), Some(archetype));
    }

    #[test]
    fn archetype_add_component() {
        let mut archetypes = Archetypes::new();
        let entity = Entity::root(0);

        archetypes.register::<Age>();
        archetypes.add_entity(entity);
        archetypes.add_component(entity, Age(0), Frame::ZERO);

        let age = archetypes.get_component::<Age>(entity);
        assert_eq!(age, Some(&Age(0)));
    }

    #[test]
    fn archetype_add_components() {
        let mut archetypes = Archetypes::new();
        let entity = Entity::root(0);

        let age = archetypes.register::<Age>();
        let name = archetypes.register::<Name>();
        archetypes.add_entity(entity);

        let mut components = Row::new();
        components.insert(age, Age(0));
        components.insert(name, Name("Bob"));
        archetypes.add_components(entity, components, Frame::ZERO);

        let age = archetypes.get_component::<Age>(entity);
        assert_eq!(age, Some(&Age(0)));

        let name = archetypes.get_component::<Name>(entity);
        assert_eq!(name, Some(&Name("Bob")));
    }

    #[test]
    fn archetype_remove_component() {
        let mut archetypes = Archetypes::new();
        let entity = Entity::root(0);

        archetypes.register::<Age>();
        archetypes.add_entity(entity);
        archetypes.add_component(entity, Age(0), Frame::ZERO);

        let age = archetypes.remove_component::<Age>(entity);
        assert_eq!(age, Some(Age(0)));
    }

    #[test]
    fn archetype_remove_components() {
        let mut archetypes = Archetypes::new();
        let entity = Entity::root(0);

        let age = archetypes.register::<Age>();
        let name = archetypes.register::<Name>();
        archetypes.add_entity(entity);

        let mut components = Row::new();
        components.insert(age, Age(0));
        components.insert(name, Name("Bob"));
        archetypes.add_components(entity, components, Frame::ZERO);

        let components = vec![age, name];
        let components = archetypes.remove_components(entity, components).unwrap();

        let age = components.get::<Age>(age);
        assert_eq!(age, Some(&Age(0)));

        let name = components.get::<Name>(name);
        assert_eq!(name, Some(&Name("Bob")));
    }

    #[test]
    fn query_include() {
        let mut archetypes = Archetypes::new();
        let entity = Entity::root(0);

        let age = archetypes.register::<Age>();
        let name = archetypes.register::<Name>();

        archetypes.add_component(entity, Age(0), Frame::ZERO);
        archetypes.add_component(entity, Name("Bob"), Frame::ZERO);

        let mut query = ArchetypeQuery::default();
        query.include(age);
        query.include(name);

        let result = archetypes.query(&query);
        let has_entity = result.iter().any(|archetype| archetype.contains(entity));

        assert!(has_entity);
    }

    #[test]
    fn query_exclude() {
        let mut archetypes = Archetypes::new();
        let entity = Entity::root(0);

        let age = archetypes.register::<Age>();
        let name = archetypes.register::<Name>();

        archetypes.add_component(entity, Age(0), Frame::ZERO);
        archetypes.add_component(entity, Name("Bob"), Frame::ZERO);

        let mut query = ArchetypeQuery::default();
        query.include(age);
        query.exclude(name);

        let result = archetypes.query(&query);
        let has_entity = result.iter().any(|archetype| archetype.contains(entity));

        assert!(!has_entity);
    }
}
