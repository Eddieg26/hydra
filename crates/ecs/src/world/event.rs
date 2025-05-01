use super::{Entity, World, WorldCell, resource::Resource};
use std::{any::TypeId, collections::HashMap};

pub trait Event: Send + Sync + Sized + 'static {}

pub type EventId = u32;
pub type EventIndex = usize;

pub struct EventStorage<E: Event> {
    pub(crate) events: Vec<E>,
    pub(crate) entities: HashMap<Entity, Vec<EventIndex>>,
}

impl<E: Event> Default for EventStorage<E> {
    fn default() -> Self {
        Self {
            events: vec![],
            entities: HashMap::new(),
        }
    }
}

pub struct Events<E: Event> {
    pub(crate) write: EventStorage<E>,
    pub(crate) read: EventStorage<E>,
}

impl<E: Event> Events<E> {
    pub fn new() -> Self {
        Self {
            write: EventStorage::default(),
            read: EventStorage::default(),
        }
    }

    pub fn update(&mut self) {
        self.read = std::mem::take(&mut self.write);
    }

    pub fn entity(&self, entity: Entity) -> std::slice::Iter<'_, usize> {
        self.read
            .entities
            .get(&entity)
            .map(|events| events.iter())
            .unwrap_or([].iter())
    }
}

impl<E: Event> Resource for Events<E> {}

pub struct EventMeta {
    pub name: &'static str,
    update: fn(&mut World),
}

pub struct EventRegistry {
    metas: Vec<EventMeta>,
    map: HashMap<TypeId, EventId>,
}

impl EventRegistry {
    pub fn new() -> Self {
        Self {
            metas: Vec::new(),
            map: HashMap::new(),
        }
    }

    pub fn register<E: Event>(&mut self) -> EventId {
        let ty = TypeId::of::<E>();
        if let Some(ty) = self.map.get(&ty).copied() {
            return ty;
        }

        let id = self.metas.len() as u32;
        let name = std::any::type_name::<E>();
        self.metas.push(EventMeta {
            name,
            update: |world| {
                let events = world.resource_mut::<Events<E>>();
                events.update();
            },
        });

        self.map.insert(ty, id);
        id
    }

    pub fn get_id<E: Event>(&self) -> Option<EventId> {
        self.map.get(&TypeId::of::<E>()).copied()
    }

    pub fn get<E: Event>(&self) -> Option<&EventMeta> {
        let ty = TypeId::of::<E>();
        self.map
            .get(&ty)
            .and_then(|&index| self.metas.get(index as usize))
    }

    pub fn update(&self, mut world: WorldCell) {
        for meta in &self.metas {
            (meta.update)(unsafe { world.get_mut() });
        }
    }
}

pub struct EventReader<'state, E: Event> {
    events: &'state Events<E>,
    index: usize,
}

impl<'state, E: Event> EventReader<'state, E> {
    pub(crate) fn new(events: &'state Events<E>) -> Self {
        Self { events, index: 0 }
    }
}

impl<'state, E: Event> Iterator for EventReader<'state, E> {
    type Item = &'state E;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index < self.events.read.events.len() {
            let event = &self.events.read.events[self.index];
            self.index += 1;
            Some(event)
        } else {
            None
        }
    }
}

impl<'state, E: Event> IntoIterator for &'state Events<E> {
    type Item = &'state E;
    type IntoIter = EventReader<'state, E>;

    fn into_iter(self) -> Self::IntoIter {
        EventReader::new(self)
    }
}

pub struct EntityEvents<'state, E: Event> {
    events: &'state Events<E>,
    indicies: std::slice::Iter<'state, EventIndex>,
}

impl<'state, E: Event> EntityEvents<'state, E> {
    pub fn new(events: &'state Events<E>, indicies: std::slice::Iter<'state, EventIndex>) -> Self {
        Self { events, indicies }
    }
}

impl<'state, E: Event> Iterator for EntityEvents<'state, E> {
    type Item = &'state E;

    fn next(&mut self) -> Option<Self::Item> {
        self.indicies
            .next()
            .copied()
            .map(|index| &self.events.read.events[index])
    }
}

pub struct EventWriter<'state, E: Event> {
    storage: &'state mut EventStorage<E>,
}

impl<'state, E: Event> EventWriter<'state, E> {
    pub fn new(storage: &'state mut EventStorage<E>) -> Self {
        Self { storage }
    }

    pub fn send(&mut self, event: E) {
        self.storage.events.push(event);
    }

    pub fn trigger(&mut self, entity: Entity, event: E) {
        let index = self.storage.events.len();
        self.storage.events.push(event);
        self.storage.entities.entry(entity).or_default().push(index);
    }

    pub fn send_batch(&mut self, events: Vec<E>) {
        self.storage.events.extend(events);
    }
}
