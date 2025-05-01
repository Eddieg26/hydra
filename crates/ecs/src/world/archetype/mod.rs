use super::{ArchetypeAccess, Component, ComponentId, ComponentKit, Components, Entity};
use crate::{
    BlobCell, ComponentBuffer, TypeMeta,
    core::{FixedBitSet, Frame, sparse::SparseIndex},
};
use std::{collections::HashMap, fmt::Debug, ptr::NonNull};

pub mod table;

pub use table::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ArchetypeId(pub u32);

impl ArchetypeId {
    pub const EMPTY: Self = Self(0);
}

impl SparseIndex for ArchetypeId {
    fn to_usize(self) -> usize {
        self.0 as usize
    }

    fn from_usize(index: usize) -> Self {
        Self(index as u32)
    }
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

    pub(crate) fn table_mut(&mut self) -> &mut Table {
        &mut self.table
    }

    pub fn contains(&self, entity: Entity) -> bool {
        self.table.contains(entity)
    }

    pub fn matches(&self, query: &ArchetypeQuery) -> bool {
        self.bitset.is_superset(&query.required)
            && self.bitset.is_disjoint(&query.excluded)
            && (query.included.is_clear() || !self.bitset.is_disjoint(&query.included))
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

    pub fn get_entity(&self, entity: Entity) -> Option<EntityIndex> {
        self.table
            .get_entity_row(entity)
            .map(|row| EntityIndex::new(self.id, row))
    }

    pub fn add_entity(&mut self, entity: Entity, row: Row) {
        self.table.add_entity(entity, row);
    }

    pub fn remove_entity(&mut self, entity: Entity) -> Option<Row> {
        self.table.remove_entity(entity)
    }

    pub(crate) fn update(&mut self, frame: Frame) {
        self.table.update(frame);
    }
}

pub struct Archetypes {
    archetypes: Vec<Archetype>,
    archetype_map: HashMap<Box<[ComponentId]>, ArchetypeId>,
    entity_map: HashMap<Entity, ArchetypeId>,
    components: Components,
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
        }
    }

    pub fn register<C: Component>(&mut self) -> ComponentId {
        self.components.register::<C>()
    }

    pub fn register_kit<C: ComponentKit>(&mut self) -> ComponentId {
        self.components.register_kit::<C>()
    }

    pub fn archetypes(&self) -> &Vec<Archetype> {
        &self.archetypes
    }

    pub fn archetypes_mut(&mut self) -> &mut Vec<Archetype> {
        &mut self.archetypes
    }

    pub fn archetype(&self, id: ArchetypeId) -> Option<&Archetype> {
        self.archetypes.get(id.0 as usize)
    }

    pub fn entity_archetype(&self, entity: Entity) -> Option<&Archetype> {
        self.entity_map
            .get(&entity)
            .and_then(|id| self.archetypes.get(id.0 as usize))
    }

    pub fn get_entity(&self, entity: Entity) -> Option<EntityIndex> {
        self.entity_archetype(entity)
            .and_then(|archetype| archetype.get_entity(entity))
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

    pub fn add_component<C: Component>(
        &mut self,
        entity: Entity,
        component: C,
        frame: Frame,
    ) -> EntityIndex {
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

        self.add_entity_inner(entity, row)
    }

    pub fn add_components(
        &mut self,
        entity: Entity,
        mut components: Row,
        frame: Frame,
    ) -> EntityIndex {
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

        self.add_entity_inner(entity, row)
    }

    pub fn add_component_kit<C: ComponentKit>(
        &mut self,
        entity: Entity,
        components: C,
        frame: Frame,
    ) -> EntityIndex {
        let (_, mut row) = match self.remove_entity(entity) {
            Some((id, row)) => (id, row),
            None => (ArchetypeId::EMPTY, Row::new()),
        };

        let mut buffer = ComponentBuffer::new();
        components.get(&mut buffer);

        let kit = self.register_kit::<C>();
        let ids = self.components().kits()[kit.to_usize()].clone();
        for id in ids.ones().map(ComponentId::from_usize) {
            let component = buffer.next().unwrap();
            let cell = match row.remove(id) {
                Some(mut cell) => {
                    cell.replace_data(component);
                    cell.modify(frame);
                    cell
                }
                None => TableCell::from_blob(component, frame),
            };

            row.insert_cell(id, cell);
        }

        self.add_entity_inner(entity, row)
    }

    pub fn remove_component<C: Component>(&mut self, entity: Entity) -> Option<(EntityIndex, C)> {
        let id = unsafe { self.components.get_id_unchecked::<C>() };

        let (_, mut row) = match self.remove_entity(entity) {
            Some(value) => value,
            None => return None,
        };

        let component = row.remove(id);

        let index = self.add_entity_inner(entity, row);

        component.map(|c| (index, c.into_value()))
    }

    pub fn remove_components(
        &mut self,
        entity: Entity,
        components: Vec<ComponentId>,
    ) -> Option<(EntityIndex, Row)> {
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

        let index = self.add_entity_inner(entity, row);

        Some((index, removed))
    }

    pub fn remove_component_kit<C: ComponentKit + Debug>(
        &mut self,
        entity: Entity,
    ) -> (Option<(EntityIndex, Row)>) {
        let (_, mut row) = match self.remove_entity(entity) {
            Some((id, row)) => (id, row),
            None => return None,
        };

        let mut removed = Row::new();
        for id in C::ids(&mut self.components) {
            if let Some(value) = row.remove(id) {
                removed.insert_cell(id, value);
            }
        }

        let index = self.add_entity_inner(entity, row);

        Some((index, removed))
    }

    #[inline]
    fn add_entity_inner(&mut self, entity: Entity, components: Row) -> EntityIndex {
        let mut ids = components.ids().to_vec();
        ids.sort();

        let id = ids.into_boxed_slice();

        match self.archetype_map.get(&id).copied() {
            Some(id) => {
                let archetype = &mut self.archetypes[id.0 as usize];
                let row = archetype.table.add_entity(entity, components);
                self.entity_map.insert(entity, id);

                EntityIndex::new(id, row)
            }
            None => {
                let mut bits = FixedBitSet::with_capacity(self.components.len());
                id.iter().for_each(|id| bits.set(id.to_usize(), true));

                let archetype_id = ArchetypeId(self.archetypes.len() as u32);
                let archetype = Archetype::new(archetype_id, components.into_table(entity), bits);

                self.archetypes.push(archetype);
                self.entity_map.insert(entity, archetype_id);
                self.archetype_map.insert(id, archetype_id);

                EntityIndex::new(archetype_id, RowIndex(0))
            }
        }
    }

    pub(crate) fn update(&mut self, frame: Frame) {
        for archetype in &mut self.archetypes {
            archetype.update(frame);
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EntityIndex {
    pub archetype: ArchetypeId,
    pub row: RowIndex,
}

impl EntityIndex {
    pub fn new(archetype: ArchetypeId, row: RowIndex) -> Self {
        Self { archetype, row }
    }
}

pub struct ArchetypeQuery {
    pub required: FixedBitSet,
    pub included: FixedBitSet,
    pub excluded: FixedBitSet,
}

impl ArchetypeQuery {
    pub fn new() -> Self {
        Self {
            required: FixedBitSet::new(),
            included: FixedBitSet::new(),
            excluded: FixedBitSet::new(),
        }
    }

    pub fn require(&mut self, component: ComponentId) {
        self.required.grow(component.to_usize() + 1);
        self.required.set(component.to_usize(), true);
    }

    pub fn include(&mut self, component: ComponentId) {
        self.included.grow(component.to_usize() + 1);
        self.included.set(component.to_usize(), true);
    }

    pub fn exclude(&mut self, component: ComponentId) {
        self.excluded.grow(component.to_usize() + 1);
        self.excluded.set(component.to_usize(), true);
    }
}

impl From<ArchetypeAccess> for ArchetypeQuery {
    fn from(value: ArchetypeAccess) -> Self {
        Self {
            required: value.required.collect(),
            included: value.includes,
            excluded: value.excludes,
        }
    }
}

#[allow(unused_imports, dead_code)]
mod tests {
    use crate::{
        ArchetypeAccess, ArchetypeQuery, ComponentKit,
        core::Frame,
        world::{Component, Entity, Row},
    };

    use super::Archetypes;

    #[derive(Debug, PartialEq, Eq)]
    struct Age(u32);
    impl Component for Age {}

    #[derive(Debug, PartialEq, Eq)]
    struct Name(&'static str);
    impl Component for Name {}

    #[derive(Debug, PartialEq, Eq)]
    struct Person {
        age: Age,
        name: Name,
    }

    impl Default for Person {
        fn default() -> Self {
            Self {
                age: Age(0),
                name: Name("Bob"),
            }
        }
    }

    impl ComponentKit for Person {
        fn ids(components: &mut crate::Components) -> Vec<crate::ComponentId> {
            vec![components.register::<Age>(), components.register::<Name>()]
        }

        fn get(self, buffer: &mut crate::ComponentBuffer) {
            buffer.push(self.age);
            buffer.push(self.name);
        }
    }

    #[test]
    fn archetype_add_entity() {
        let mut archetypes = Archetypes::new();
        let entity = Entity::root(0);
        let archetype = archetypes.add_entity(entity);

        assert_eq!(
            archetypes.entity_archetype(entity).map(|a| a.id),
            Some(archetype)
        );
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
    fn add_component_kit() {
        let mut archetypes = Archetypes::new();
        let entity = Entity::root(0);

        archetypes.add_entity(entity);
        archetypes.add_component_kit(entity, Person::default(), Frame::ZERO);

        let age = archetypes.get_component::<Age>(entity);
        assert_eq!(age, Some(&Age(0)));

        let name = archetypes.get_component::<Name>(entity);
        assert_eq!(name, Some(&Name("Bob")));
    }

    #[test]
    fn remove_component_kit() {
        let mut archetypes = Archetypes::new();
        let entity = Entity::root(0);

        let age = archetypes.register::<Age>();
        let name = archetypes.register::<Name>();
        archetypes.add_entity(entity);
        archetypes.add_component_kit(entity, Person::default(), Frame::ZERO);

        let (_, components) = archetypes.remove_component_kit::<Person>(entity).unwrap();
        let age = components.get::<Age>(age);
        assert_eq!(age, Some(&Age(0)));

        let name = components.get::<Name>(name);
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
        assert_eq!(age.map(|(_, a)| a), Some(Age(0)));
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
        let components = archetypes.remove_components(entity, components).unwrap().1;

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

        let mut query = ArchetypeQuery::new();
        query.require(age);
        query.require(name);

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

        let mut query = ArchetypeQuery::new();
        query.require(age);
        query.exclude(name);

        let result = archetypes.query(&query);
        let has_entity = result.iter().any(|archetype| archetype.contains(entity));

        assert!(!has_entity);
    }
}
