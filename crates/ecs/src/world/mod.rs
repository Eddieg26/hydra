use crate::{SparseIndex, core::Frame, impl_sparse_index_wrapper, system::Removed};
use std::{
    any::TypeId,
    collections::HashMap,
    ops::{Index, IndexMut},
};

pub mod access;
pub mod archetype;
pub mod cell;
pub mod component;
pub mod entity;
pub mod event;
pub mod resource;

pub use access::*;
pub use archetype::*;
pub use cell::*;
pub use component::*;
pub use entity::*;
pub use event::*;
pub use resource::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorldId(pub u32);
impl WorldId {
    fn new() -> Self {
        static mut ID: u32 = 0;
        unsafe {
            let id = ID;
            ID += 1;
            WorldId(id)
        }
    }
}

pub struct World {
    pub(crate) id: WorldId,
    pub(crate) archetypes: Archetypes,
    pub(crate) resources: Resources,
    pub(crate) entities: Entities,
    pub(crate) events: EventRegistry,
    pub(crate) modes: WorldModeCategories,
    pub(crate) frame: Frame,
}

impl World {
    pub fn new() -> Self {
        World {
            id: WorldId::new(),
            archetypes: Archetypes::new(),
            resources: Resources::new(),
            entities: Entities::new(),
            events: EventRegistry::new(),
            modes: WorldModeCategories(HashMap::new()),
            frame: Frame(1),
        }
    }

    pub fn id(&self) -> WorldId {
        self.id
    }

    pub fn components(&self) -> &Components {
        self.archetypes.components()
    }

    pub fn archetypes(&self) -> &Archetypes {
        &self.archetypes
    }

    pub fn resources(&self) -> &Resources {
        &self.resources
    }

    pub fn entities(&self) -> &Entities {
        &self.entities
    }

    pub fn events(&self) -> &EventRegistry {
        &self.events
    }

    pub fn frame(&self) -> Frame {
        self.frame
    }

    pub fn register<C: Component>(&mut self) -> ComponentId {
        let id = self.archetypes.register::<C>();

        self.register_event::<Removed<C>>();

        id
    }

    pub fn register_resource<R: Resource + Send>(&mut self) -> ResourceId {
        self.resources.register::<true, R>()
    }

    pub fn register_non_send_resource<R: Resource>(&mut self) -> ResourceId {
        self.resources.register::<false, R>()
    }

    pub fn register_event<E: Event>(&mut self) -> EventId {
        if !self.resources.contains::<Events<E>>() {
            self.add_resource(Events::<E>::new());
        }

        self.events.register::<E>()
    }

    pub fn add_mode<M: WorldMode>(&mut self) -> ModeId {
        self.modes
            .entry(M::CATEGORY)
            .or_insert(WorldModes::new())
            .add_mode::<M>()
    }

    pub fn add_resource<R: Resource + Send>(&mut self, resource: R) -> ResourceId {
        self.resources
            .add_with_frame::<true, R>(resource, self.frame)
    }

    pub fn add_non_send_resource<R: Resource>(&mut self, resource: R) -> ResourceId {
        self.resources
            .add_with_frame::<false, R>(resource, self.frame)
    }

    pub fn resource<R: Resource + Send>(&self) -> &R {
        self.resources
            .get_id::<R>()
            .and_then(|id| self.resources.get::<R>(id))
            .expect(&format!(
                "Resource not found: {}",
                std::any::type_name::<R>()
            ))
    }

    pub fn resource_mut<R: Resource + Send>(&mut self) -> &mut R {
        self.resources
            .get_id::<R>()
            .and_then(|id| self.resources.get_mut::<R>(id, self.frame))
            .expect(&format!(
                "Resource not found: {}",
                std::any::type_name::<R>()
            ))
    }

    pub fn try_resource<R: Resource + Send>(&self) -> Option<&R> {
        self.resources
            .get_id::<R>()
            .and_then(|id| self.resources.get::<R>(id))
    }

    pub fn try_resource_mut<R: Resource + Send>(&mut self) -> Option<&mut R> {
        self.resources
            .get_id::<R>()
            .and_then(|id| self.resources.get_mut::<R>(id, self.frame))
    }

    pub fn non_send_resource<R: Resource>(&self) -> &R {
        self.resources
            .get_id::<R>()
            .and_then(|id| self.resources.get::<R>(id))
            .expect(&format!(
                "Non Send Resource not found: {}",
                std::any::type_name::<R>()
            ))
    }

    pub fn non_send_resource_mut<R: Resource>(&mut self) -> &mut R {
        self.resources
            .get_id::<R>()
            .and_then(|id| self.resources.get_mut::<R>(id, self.frame))
            .expect(&format!(
                "Non Send Resource not found: {}",
                std::any::type_name::<R>()
            ))
    }

    pub fn try_non_send_resource<R: Resource>(&self) -> Option<&R> {
        self.resources
            .get_id::<R>()
            .and_then(|id| self.resources.get::<R>(id))
    }

    pub fn try_non_send_resource_mut<R: Resource>(&mut self) -> Option<&mut R> {
        self.resources
            .get_id::<R>()
            .and_then(|id| self.resources.get_mut::<R>(id, self.frame))
    }

    pub fn get_or_insert_resource<R: Resource + Send>(&mut self, get: impl Fn() -> R) -> &mut R {
        unsafe {
            let mut world = self.cell();
            if let Some(resource) = world.get_mut().try_resource_mut::<R>() {
                resource
            } else {
                let world = world.get_mut();
                let id = world.add_resource(get());
                world.resources.get_mut(id, self.frame).unwrap()
            }
        }
    }

    pub fn get_or_insert_non_send_resource<R: Resource>(&mut self, get: impl Fn() -> R) -> &mut R {
        unsafe {
            let mut world = self.cell();
            if let Some(resource) = world.get_mut().try_non_send_resource_mut::<R>() {
                resource
            } else {
                let world = world.get_mut();
                let id = world.add_non_send_resource(get());
                world.resources.get_mut(id, self.frame).unwrap()
            }
        }
    }

    pub fn remove_resource<R: Resource>(&mut self) -> Option<R> {
        self.resources.remove::<R>(self.frame)
    }

    pub fn remove_resource_by_id(&mut self, id: ResourceId) -> Option<Vec<u8>> {
        self.resources.remove_by_id(id, self.frame)
    }

    pub unsafe fn cell(&'_ self) -> WorldCell<'_> {
        unsafe { WorldCell::new(self) }
    }
}

impl World {
    pub fn spawn(&mut self) -> Entity {
        let entity = self.entities.spawn();
        self.archetypes.add_entity(entity);
        entity
    }

    pub fn despawn(&mut self, entity: Entity) -> Option<Row> {
        self.entities.despawn(entity);
        self.archetypes.remove_entity(entity)
    }

    pub fn has_component<C: Component>(&self, entity: Entity) -> bool {
        let Some(component) = self.archetypes.components().get_id::<C>() else {
            return false;
        };

        self.archetypes
            .entity_archetype(entity)
            .map(|a| a.has_component(component.to_usize()))
            .unwrap_or(false)
    }

    pub fn get_component<C: Component>(&self, entity: Entity) -> Option<&C> {
        self.archetypes.get_component::<C>(entity)
    }

    pub fn get_component_mut<C: Component>(&mut self, entity: Entity) -> Option<&mut C> {
        self.archetypes.get_component_mut::<C>(entity)
    }

    pub fn add_component<C: Component>(&mut self, entity: Entity, component: C) -> EntityIndex {
        self.archetypes
            .add_component(entity, component, self.frame + Frame(1))
    }

    pub fn remove_component<C: Component>(&mut self, entity: Entity) -> Option<EntityIndex> {
        let (index, component) = self.archetypes.remove_component::<C>(entity)?;
        let events = self.get_or_insert_resource(|| Events::<Removed<C>>::new());
        events
            .write
            .add_entity_event(entity, Removed::from(component));

        Some(index)
    }

    pub fn add_components<C: ComponentKit>(&mut self, entity: Entity, components: C) {
        self.archetypes
            .add_components(entity, components, self.frame);
    }

    pub fn remove_components<C: ComponentKit>(&mut self, entity: Entity) -> Option<EntityIndex> {
        pub struct Remover<'a> {
            world: &'a mut World,
            entity: Entity,
            components: Row,
        }

        impl<'a> ComponentRemover for Remover<'a> {
            fn remove<C: Component>(&mut self) {
                let id = unsafe { self.world.components().get_id_unchecked::<C>() };
                if let Some(cell) = self.components.remove(id) {
                    let events = self
                        .world
                        .get_or_insert_resource(|| Events::<Removed<C>>::new());
                    events
                        .write
                        .add_entity_event(self.entity, Removed::from(cell.into_value::<C>()));
                }
            }
        }

        let (index, components) = self.archetypes.remove_components::<C>(entity)?;

        let mut remover = Remover {
            world: self,
            entity,
            components,
        };

        C::remove(&mut remover);

        Some(index)
    }

    pub fn send<E: Event>(&mut self, event: E) {
        let events = self.resource_mut::<Events<E>>();
        events.writer().send(event);
    }

    pub fn try_send<E: Event>(&mut self, event: E) -> bool {
        if let Some(events) = self.try_resource_mut::<Events<E>>() {
            events.writer().send(event);
            true
        } else {
            false
        }
    }

    pub fn entity_mut(&'_ mut self, entity: Entity) -> EntityMut<'_> {
        let index = self.archetypes.get_entity(entity).unwrap();
        EntityMut::new(self, entity, index)
    }

    pub fn enter<M: WorldMode>(&mut self) -> Option<ModeId> {
        let mut modes = self.modes.remove(M::CATEGORY)?;
        let current = modes.add_mode::<M>();

        let id = if modes.current != Some(current) {
            if let Some(prev) = modes.current {
                modes.get(prev).exit(self);
                modes[prev].set_frame(self.frame);
            }

            modes.get(current).enter(self);
            modes[current].set_frame(self.frame);
            modes.current.replace(current)
        } else {
            None
        };

        self.modes.insert(M::CATEGORY, modes);
        id
    }

    pub fn exit(&mut self, category: &str) -> bool {
        let Some((category, mut modes)) = self.modes.remove_entry(category) else {
            return false;
        };

        let exited = if let Some(prev) = modes.current {
            modes.get(prev).exit(self);
            modes.current = None;
            modes[prev].set_frame(self.frame);
            true
        } else {
            false
        };

        self.modes.insert(category, modes);
        exited
    }

    pub fn update(&mut self) {
        self.frame += 1;
        self.events.update(unsafe { self.cell() });

        if self.frame.get() % Frame::AGE_REFRESH_RATE == 0 {
            self.archetypes.update(self.frame);
            self.resources.update(self.frame);
            self.modes.update(self.frame);
        }
    }
}

impl Default for World {
    fn default() -> Self {
        Self::new()
    }
}

pub struct EntityMut<'w> {
    pub(crate) world: &'w mut World,
    entity: Entity,
    index: EntityIndex,
}

impl<'w> EntityMut<'w> {
    fn new(world: &'w mut World, entity: Entity, index: EntityIndex) -> Self {
        Self {
            world,
            entity,
            index,
        }
    }

    pub fn id(&self) -> Entity {
        self.entity
    }

    pub fn index(&self) -> EntityIndex {
        self.index
    }

    pub fn add_component<C: Component>(&mut self, component: C) -> &mut Self {
        self.index = self.world.add_component(self.entity, component);
        self
    }

    pub fn remove_component<C: Component>(&mut self) {
        if let Some(index) = self.world.remove_component::<C>(self.entity) {
            self.index = index;
        }
    }

    pub fn get_component<C: Component>(&self) -> Option<&C> {
        let component = self.world.components().get_id::<C>()?;
        self.world.archetypes[self.index.archetype]
            .table()
            .get_row_component::<C>(self.index.row, component)
    }

    pub fn get_component_mut<C: Component>(&mut self) -> Option<&mut C> {
        let component = self.world.components().get_id::<C>()?;
        self.world.archetypes[self.index.archetype]
            .table_mut()
            .get_row_component_mut::<C>(self.index.row, component)
    }
}

pub trait WorldMode: Send + Sync + 'static {
    const CATEGORY: &'static str;

    fn enter(_: &mut World) {}
    fn exit(_: &mut World) {}
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ModeId(pub u32);
impl_sparse_index_wrapper!(crate, ModeId);

#[derive(Clone, Copy)]
pub struct BoxedMode {
    enter: fn(&mut World),
    exit: fn(&mut World),
    frame: Frame,
}

impl BoxedMode {
    pub fn new<M: WorldMode>() -> Self {
        Self {
            enter: M::enter,
            exit: M::exit,
            frame: Frame::ZERO,
        }
    }

    pub fn frame(&self) -> Frame {
        self.frame
    }

    pub fn set_frame(&mut self, frame: Frame) {
        self.frame = frame;
    }

    pub(super) fn enter(&self, world: &mut World) {
        (self.enter)(world);
    }

    pub(super) fn exit(&self, world: &mut World) {
        (self.exit)(world);
    }
}

pub struct WorldModeCategories(HashMap<&'static str, WorldModes>);

impl WorldModeCategories {
    pub fn update(&mut self, frame: Frame) {
        for modes in self.0.values_mut() {
            modes.update(frame);
        }
    }
}

impl std::ops::Deref for WorldModeCategories {
    type Target = HashMap<&'static str, WorldModes>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for WorldModeCategories {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

pub struct WorldModes {
    modes: Vec<BoxedMode>,
    map: HashMap<TypeId, ModeId>,
    pub(super) current: Option<ModeId>,
}

impl WorldModes {
    pub fn new() -> Self {
        Self {
            modes: Vec::new(),
            map: HashMap::new(),
            current: None,
        }
    }

    pub fn id<M: WorldMode>(&self) -> Option<ModeId> {
        self.map.get(&TypeId::of::<M>()).copied()
    }

    pub fn current(&self) -> Option<ModeId> {
        self.current
    }

    pub fn get(&self, id: ModeId) -> BoxedMode {
        self.modes[id.to_usize()]
    }

    pub fn add_mode<M: WorldMode>(&mut self) -> ModeId {
        let ty = TypeId::of::<M>();
        match self.map.get(&ty).copied() {
            Some(id) => id,
            None => {
                let id = ModeId::from_usize(self.modes.len());
                self.modes.push(BoxedMode::new::<M>());
                self.map.insert(ty, id);
                id
            }
        }
    }

    pub(super) fn update(&mut self, frame: Frame) {
        for mode in &mut self.modes {
            mode.frame.update(frame);
        }
    }
}

impl Index<ModeId> for WorldModes {
    type Output = BoxedMode;

    fn index(&self, index: ModeId) -> &Self::Output {
        &self.modes[index.to_usize()]
    }
}

impl IndexMut<ModeId> for WorldModes {
    fn index_mut(&mut self, index: ModeId) -> &mut Self::Output {
        &mut self.modes[index.to_usize()]
    }
}
