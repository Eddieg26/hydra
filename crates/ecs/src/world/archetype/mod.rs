use super::{ArchetypeAccess, Component, ComponentId, ComponentKit, Components, Entity, Event};
use crate::{
    ComponentWriter,
    core::{FixedBitSet, Frame, sparse::SparseIndex},
};
use std::collections::HashMap;

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
    edges: Edges,
}

impl Archetype {
    pub fn new(id: ArchetypeId, table: Table, bitset: FixedBitSet) -> Self {
        Self {
            id,
            table,
            bitset,
            edges: Edges::new(),
        }
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

    pub fn edges(&self) -> &Edges {
        &self.edges
    }

    pub fn edges_mut(&mut self) -> &mut Edges {
        &mut self.edges
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

    pub fn add_entity(&mut self, entity: Entity, row: Row) -> EntityIndex {
        let row = self.table.add_entity(entity, row);

        EntityIndex::new(self.id, row)
    }

    pub fn remove_entity(&mut self, entity: Entity) -> Option<Row> {
        self.table.remove_entity(entity)
    }

    pub(crate) fn update(&mut self, frame: Frame) {
        self.table.update(frame);
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum ArchetypeEdgeId {
    Component(ComponentId),
    Kit(ComponentId),
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum Edge {
    Add,
    Remove,
}

pub struct Edges {
    pub add: HashMap<ArchetypeEdgeId, ArchetypeId>,
    pub remove: HashMap<ArchetypeEdgeId, ArchetypeId>,
}

impl Edges {
    pub fn new() -> Self {
        Self {
            add: HashMap::new(),
            remove: HashMap::new(),
        }
    }

    pub fn get(&self, id: ArchetypeEdgeId, edge: Edge) -> Option<ArchetypeId> {
        match edge {
            Edge::Add => self.added(id),
            Edge::Remove => self.removed(id),
        }
    }

    pub fn insert(&mut self, id: ArchetypeEdgeId, archetype: ArchetypeId, edge: Edge) {
        match edge {
            Edge::Add => self.add.insert(id, archetype),
            Edge::Remove => self.remove.insert(id, archetype),
        };
    }

    pub fn added(&self, id: ArchetypeEdgeId) -> Option<ArchetypeId> {
        self.add.get(&id).copied()
    }

    pub fn removed(&self, id: ArchetypeEdgeId) -> Option<ArchetypeId> {
        self.remove.get(&id).copied()
    }
}

pub struct Archetypes {
    archetypes: Vec<Archetype>,
    archetype_map: HashMap<FixedBitSet, ArchetypeId>,
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

        let mut archetype_map = HashMap::new();
        archetype_map.insert(FixedBitSet::new(), ArchetypeId::EMPTY);

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

    pub fn remove_entity(&mut self, entity: Entity) -> Option<Row> {
        let id = self.entity_map.remove(&entity)?;
        let archetype = &mut self.archetypes[id.0 as usize];
        let row = archetype.remove_entity(entity)?;

        Some(row)
    }

    pub fn get_component<C: Component>(&self, entity: Entity) -> Option<&C> {
        let id = unsafe { self.components.get_id_unchecked::<C>() };
        let archetype_id = self.entity_map.get(&entity)?;
        let archetype = &self.archetypes[archetype_id.to_usize()];
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
        let id = self.components.register::<C>();
        let archetype = self.get_or_insert_entity(entity);
        let next = self.next_added_archetype(archetype, ArchetypeEdgeId::Component(id));

        if archetype == next {
            let table = &mut self.archetypes[archetype.to_usize()].table;
            let index = table.get_entity_row(entity).unwrap();
            let column = table.get_column_mut(id).unwrap();

            column.replace(index.to_usize(), component, frame);

            EntityIndex::new(archetype, index)
        } else {
            let mut row = self.remove_entity(entity).unwrap();
            row.insert(id, component, frame);

            self.entity_map.insert(entity, next);
            self.archetypes[next.to_usize()].add_entity(entity, row)
        }
    }

    pub fn add_components<C: ComponentKit>(
        &mut self,
        entity: Entity,
        components: C,
        frame: Frame,
    ) -> EntityIndex {
        let kit = self.components.register_kit::<C>();
        let archetype = self.get_or_insert_entity(entity);
        let next = self.next_added_archetype(archetype, ArchetypeEdgeId::Kit(kit));

        if archetype == next {
            let table = &mut self.archetypes[archetype.to_usize()].table;
            let index = table.get_entity_row(entity).unwrap();
            let bits = &self.components.kits()[kit.to_usize()];

            pub struct ComponentModifier<'a> {
                table: &'a mut Table,
                components: fixedbitset::Ones<'a>,
                index: RowIndex,
                frame: Frame,
            }

            impl<'a> ComponentWriter<'a> for ComponentModifier<'a> {
                fn write<C: Component>(&mut self, component: C) {
                    let id = self.components.next().map(ComponentId::from_usize).unwrap();
                    self.table.get_column_mut(id).unwrap().replace::<C>(
                        self.index.to_usize(),
                        component,
                        self.frame,
                    );
                }
            }

            let writer = ComponentModifier {
                table,
                components: bits.ones(),
                index,
                frame,
            };

            components.get(writer);

            EntityIndex::new(archetype, index)
        } else {
            let mut row = self.remove_entity(entity).unwrap();
            let table = &mut self.archetypes[next.to_usize()].table;
            let bits = &self.components.kits()[kit.to_usize()];

            pub struct ComponentAdder<'a> {
                row: &'a mut Row,
                components: fixedbitset::Ones<'a>,
                frame: Frame,
            }

            impl<'a> ComponentWriter<'a> for ComponentAdder<'a> {
                fn write<C: Component>(&mut self, component: C) {
                    let id = self.components.next().map(ComponentId::from_usize).unwrap();
                    self.row.insert(id, component, self.frame);
                }
            }

            let writer = ComponentAdder {
                row: &mut row,
                components: bits.ones(),
                frame,
            };

            components.get(writer);

            let index = table.add_entity(entity, row);

            self.entity_map.insert(entity, next);
            EntityIndex::new(next, index)
        }
    }

    pub fn remove_component<C: Component>(&mut self, entity: Entity) -> Option<(EntityIndex, C)> {
        let id = self.components.register::<C>();
        let archetype = self.get_or_insert_entity(entity);
        let next = self.next_removed_archetype(archetype, ArchetypeEdgeId::Component(id));

        if archetype == next {
            None
        } else {
            let mut row = self.remove_entity(entity).unwrap();
            let component = row.remove(id)?.into_value::<C>();
            let index = self.archetypes[next.to_usize()].add_entity(entity, row);

            self.entity_map.insert(entity, next);
            Some((index, component))
        }
    }

    pub fn remove_components<C: ComponentKit>(
        &mut self,
        entity: Entity,
    ) -> Option<(EntityIndex, Row)> {
        let kit = self.components.register_kit::<C>();
        let archetype = self.get_or_insert_entity(entity);
        let next = self.next_removed_archetype(archetype, ArchetypeEdgeId::Kit(kit));

        if archetype == next {
            None
        } else {
            let mut row = self.remove_entity(entity).unwrap();
            let mut removed = Row::new();
            for id in self.components.kits()[kit.to_usize()]
                .ones()
                .map(ComponentId::from_usize)
            {
                removed.insert_cell(id, row.remove(id).unwrap());
            }

            let index = self.archetypes[next.to_usize()].add_entity(entity, row);

            self.entity_map.insert(entity, next);
            Some((index, removed))
        }
    }

    #[inline]
    fn get_or_insert_entity(&mut self, entity: Entity) -> ArchetypeId {
        match self.entity_map.get(&entity).copied() {
            Some(id) => id,
            None => {
                self.entity_map.insert(entity, ArchetypeId::EMPTY);
                self.archetypes[ArchetypeId::EMPTY.to_usize()].add_entity(entity, Row::new());
                ArchetypeId::EMPTY
            }
        }
    }

    #[inline]
    fn next_added_archetype(&mut self, current: ArchetypeId, edge: ArchetypeEdgeId) -> ArchetypeId {
        match self.archetypes[current.to_usize()].edges.added(edge) {
            Some(id) => id,
            None => {
                let mut bits = self.archetypes[current.to_usize()].bitset.clone();
                match edge {
                    ArchetypeEdgeId::Component(id) => {
                        bits.grow(id.to_usize() + 1);
                        bits.set(id.to_usize(), true);
                    }
                    ArchetypeEdgeId::Kit(id) => {
                        bits.union_with(&self.components.kits()[id.to_usize()])
                    }
                }

                if self.archetypes[current.to_usize()].bitset == bits {
                    return current;
                }

                let mut table = TableBuilder::new();
                for id in bits.ones().map(ComponentId::from_usize) {
                    let component = self.components.get_by_id(id).unwrap();
                    table.add_raw_column(id, *component.meta());
                }

                let next_id = ArchetypeId::from_usize(self.archetypes.len());
                let mut archetype = Archetype::new(next_id, table.build(), bits.clone());

                archetype.edges.remove.insert(edge, current);
                self.archetypes[current.to_usize()]
                    .edges
                    .add
                    .insert(edge, next_id);

                self.archetypes.push(archetype);
                self.archetype_map.insert(bits, next_id);

                next_id
            }
        }
    }

    #[inline]
    fn next_removed_archetype(
        &mut self,
        current: ArchetypeId,
        edge: ArchetypeEdgeId,
    ) -> ArchetypeId {
        match self.archetypes[current.to_usize()].edges.added(edge) {
            Some(id) => id,
            None => {
                let mut bits = self.archetypes[current.to_usize()].bitset.clone();
                match edge {
                    ArchetypeEdgeId::Component(id) => {
                        bits.grow(id.to_usize() + 1);
                        bits.set(id.to_usize(), false);
                    }
                    ArchetypeEdgeId::Kit(id) => {
                        bits.difference_with(&self.components.kits()[id.to_usize()]);
                    }
                }

                if self.archetypes[current.to_usize()].bitset == bits {
                    return current;
                }

                let mut table = TableBuilder::new();
                for id in bits.ones().map(ComponentId::from_usize) {
                    let component = self.components.get_by_id(id).unwrap();
                    table.add_raw_column(id, *component.meta());
                }

                let next_id = ArchetypeId::from_usize(self.archetypes.len());
                let mut archetype = Archetype::new(next_id, table.build(), bits.clone());

                archetype.edges.add.insert(edge, current);
                self.archetypes[current.to_usize()]
                    .edges
                    .remove
                    .insert(edge, next_id);

                self.archetypes.push(archetype);
                self.archetype_map.insert(bits, next_id);

                next_id
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

pub struct RemovedComponent<C: Component> {
    component: C,
}

impl<C: Component> RemovedComponent<C> {
    pub fn new(component: C) -> Self {
        Self { component }
    }

    pub fn take(self) -> C {
        self.component
    }
}

impl<C: Component> Event for RemovedComponent<C> {}

#[allow(unused_imports, dead_code)]
mod tests {
    use std::hash::{DefaultHasher, Hash, Hasher};

    use crate::{
        ArchetypeAccess, ArchetypeQuery, ComponentId, ComponentKit, SparseIndex,
        core::Frame,
        world::{Component, Entity, Row},
    };

    use super::{ArchetypeEdgeId, Archetypes};

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

        fn get<'a>(self, mut writer: impl crate::ComponentWriter<'a>) {
            writer.write(self.age);
            writer.write(self.name);
        }

        fn remove<'a>(mut remover: impl crate::ComponentRemover<'a>) {
            remover.remove::<Age>();
            remover.remove::<Name>();
        }
    }

    #[test]
    fn add_entity() {
        let mut archetypes = Archetypes::new();
        let entity = Entity::root(0);
        let archetype = archetypes.add_entity(entity);

        assert_eq!(
            archetypes.entity_archetype(entity).map(|a| a.id),
            Some(archetype)
        );
    }

    #[test]
    fn add_component() {
        let mut archetypes = Archetypes::new();
        let entity = Entity::root(0);

        archetypes.register::<Age>();
        archetypes.add_component(entity, Age(0), Frame::ZERO);

        let age = archetypes.get_component::<Age>(entity);
        assert_eq!(age, Some(&Age(0)));
    }

    #[test]
    fn add_components() {
        let mut archetypes = Archetypes::new();
        let entity = Entity::root(0);

        archetypes.add_entity(entity);
        archetypes.add_components(entity, Person::default(), Frame::ZERO);

        let age = archetypes.get_component::<Age>(entity);
        assert_eq!(age, Some(&Age(0)));

        let name = archetypes.get_component::<Name>(entity);
        assert_eq!(name, Some(&Name("Bob")));
    }

    #[test]
    fn remove_component() {
        let mut archetypes = Archetypes::new();
        let entity = Entity::root(0);

        archetypes.register::<Age>();
        archetypes.add_entity(entity);
        archetypes.add_component(entity, Age(0), Frame::ZERO);

        let age = archetypes.remove_component::<Age>(entity);
        assert_eq!(age.map(|(_, a)| a), Some(Age(0)));
    }

    #[test]
    fn remove_components() {
        let mut archetypes = Archetypes::new();
        let entity = Entity::root(0);

        let age = archetypes.register::<Age>();
        let name = archetypes.register::<Name>();
        archetypes.add_entity(entity);
        archetypes.add_components(entity, Person::default(), Frame::ZERO);

        let (_, components) = archetypes.remove_components::<Person>(entity).unwrap();
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
