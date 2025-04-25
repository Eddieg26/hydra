use crate::core::Frame;

pub mod archetype;
pub mod cell;
pub mod component;
pub mod entity;
pub mod event;
pub mod resource;

pub use archetype::*;
pub use cell::*;
pub use component::*;
pub use entity::*;
pub use event::*;
pub use resource::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorldId(u32);
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

    pub fn entities_mut(&mut self) -> &mut Entities {
        &mut self.entities
    }

    pub fn events(&self) -> &EventRegistry {
        &self.events
    }

    pub fn frame(&self) -> Frame {
        self.frame
    }

    pub fn register<C: Component>(&mut self) -> ComponentId {
        self.archetypes.register::<C>()
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

    pub fn add_resource<R: Resource + Send>(&mut self, resource: R) {
        self.resources.add::<true, R>(resource);
    }

    pub fn add_non_send_resource<R: Resource>(&mut self, resource: R) {
        self.resources.add::<false, R>(resource);
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
            .and_then(|id| self.resources.get_mut::<R>(id))
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
            .and_then(|id| self.resources.get_mut::<R>(id))
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
            .and_then(|id| self.resources.get_mut::<R>(id))
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
            .and_then(|id| self.resources.get_mut::<R>(id))
    }

    pub fn remove_resource<R: Resource>(&mut self) -> Option<R> {
        self.resources.remove::<R>()
    }

    pub unsafe fn cell(&self) -> WorldCell {
        unsafe { WorldCell::new(self) }
    }
}

impl World {
    pub fn spawn(&mut self) -> Entity {
        let entity = self.entities.spawn();
        self.archetypes.add_entity(entity);
        entity
    }

    pub fn despawn(&mut self, entity: Entity) -> Option<(ArchetypeId, Row)> {
        self.entities.despawn(entity);
        self.archetypes.remove_entity(entity)
    }

    pub fn get_component<C: Component>(&self, entity: Entity) -> Option<&C> {
        self.archetypes.get_component::<C>(entity)
    }

    pub fn get_component_mut<C: Component>(&mut self, entity: Entity) -> Option<&mut C> {
        self.archetypes.get_component_mut::<C>(entity)
    }

    pub fn add_component<C: Component>(&mut self, entity: Entity, component: C) -> EntityIndex {
        self.archetypes.add_component(entity, component, self.frame)
    }

    pub fn remove_component<C: Component>(&mut self, entity: Entity) -> Option<(EntityIndex, C)> {
        self.archetypes.remove_component::<C>(entity)
    }

    pub fn add_components(&mut self, entity: Entity, components: Row) {
        self.archetypes
            .add_components(entity, components, self.frame);
    }

    pub fn remove_components(&mut self, entity: Entity, components: Vec<ComponentId>) {
        self.archetypes.remove_components(entity, components);
    }

    pub fn entity_mut(&mut self, entity: Entity) -> EntityMut {
        let index = self.archetypes.get_entity(entity).unwrap();
        EntityMut::new(self, entity, index)
    }

    pub fn entity_ref(&self, entity: Entity) -> EntityWorldRef {
        let index = self.archetypes.get_entity(entity).unwrap();
        EntityWorldRef::new(self, entity, index)
    }

    pub fn update(&mut self) {
        self.frame += 1;
        self.events.update(unsafe { self.cell() });

        if self.frame.get() % Frame::AGE_REFRESH_RATE == 0 {
            self.archetypes.update(self.frame);
            self.resources.update(self.frame);
        }
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

    pub fn remove_component<C: Component>(&mut self) -> Option<C> {
        let (index, component) = self.world.remove_component::<C>(self.entity)?;
        self.index = index;
        Some(component)
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

pub struct EntityWorldRef<'w> {
    world: &'w World,
    entity: Entity,
    index: EntityIndex,
}

impl<'w> EntityWorldRef<'w> {
    fn new(world: &'w World, entity: Entity, index: EntityIndex) -> Self {
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

    pub fn get_component<C: Component>(&self) -> Option<&C> {
        let component = self.world.components().get_id::<C>()?;
        self.world.archetypes[self.index.archetype]
            .table()
            .get_row_component::<C>(self.index.row, component)
    }
}
