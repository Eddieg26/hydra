use super::{World, WorldCell, resource::Resource};
use crate::{system::arg::SystemArg, SystemInit};
use std::{any::TypeId, collections::HashMap};

pub trait Event: Send + Sync + Sized + 'static {}

pub struct Events<E: Event> {
    write: Vec<E>,
    read: Vec<E>,
}

impl<E: Event> Events<E> {
    pub fn new() -> Self {
        Self {
            write: Vec::new(),
            read: Vec::new(),
        }
    }

    pub fn update(&mut self) {
        self.read = std::mem::take(&mut self.write);
    }
}

impl<E: Event> Resource for Events<E> {}

pub struct EventMeta {
    pub name: &'static str,
    update: fn(&mut World),
}

pub struct EventRegistry {
    metas: Vec<EventMeta>,
    map: HashMap<TypeId, usize>,
}

impl EventRegistry {
    pub fn new() -> Self {
        Self {
            metas: Vec::new(),
            map: HashMap::new(),
        }
    }

    pub fn register<E: Event>(&mut self) {
        let ty = TypeId::of::<E>();
        if self.map.contains_key(&ty) {
            return;
        }

        let name = std::any::type_name::<E>();
        let index = self.metas.len();
        self.metas.push(EventMeta {
            name,
            update: |world| {
                let events = world.resource_mut::<Events<E>>();
                events.update();
            },
        });

        self.map.insert(ty, index);
    }

    pub fn get<E: Event>(&self) -> Option<&EventMeta> {
        let ty = TypeId::of::<E>();
        self.map.get(&ty).and_then(|&index| self.metas.get(index))
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
        if self.index < self.events.read.len() {
            let event = &self.events.read[self.index];
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

unsafe impl<E: Event> SystemArg for EventReader<'_, E> {
    type Item<'world, 'state> = EventReader<'world, E>;

    type State = ();

    fn init(system: &mut SystemInit) -> Self::State {
        system.world().register_event::<E>();
        ()
    }

    unsafe fn get<'world, 'state>(
        _: &'state mut Self::State,
        world: super::WorldCell<'world>,
        _: &crate::system::SystemMeta,
    ) -> Self::Item<'world, 'state> {
        let events = unsafe { world.get().resource::<Events<E>>() };
        EventReader::new(events)
    }
}

pub struct EventWriter<'state, E: Event> {
    events: &'state mut Vec<E>,
}

impl<'state, E: Event> EventWriter<'state, E> {
    pub fn new(events: &'state mut Vec<E>) -> Self {
        Self { events }
    }

    pub fn send(&mut self, event: E) {
        self.events.push(event);
    }

    pub fn send_batch(&mut self, events: Vec<E>) {
        self.events.extend(events);
    }
}

unsafe impl<E: Event> SystemArg for EventWriter<'_, E> {
    type Item<'world, 'state> = EventWriter<'state, E>;

    type State = Vec<E>;

    fn init(system: &mut SystemInit) -> Self::State {
        system.world().register_event::<E>();
        vec![]
    }

    unsafe fn get<'world, 'state>(
        state: &'state mut Self::State,
        _: super::WorldCell<'world>,
        _: &crate::system::SystemMeta,
    ) -> Self::Item<'world, 'state> {
        EventWriter::new(state)
    }

    fn apply(state: &mut Self::State, world: &mut super::World) {
        let events = world.resource_mut::<Events<E>>();
        events.write.append(state);
    }
}
