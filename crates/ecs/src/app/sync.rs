use crate::{
    AppTag, Commands, Component, Despawn, Despawned, Entity, EventReader, Extract, Plugin,
    system::{Added, Main, Modified, Or, Removed},
    unlifetime::SQuery,
};
use derive_ecs::Resource;
use std::collections::HashMap;

/// A resource that maps entities in the main world to entities in the sub world.
/// This is used to synchronize components between the two worlds.
#[derive(Resource, Default)]
pub struct EntityWorldMap(HashMap<Entity, Entity>);

#[derive(Component, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MainEntity(pub Entity);
impl std::ops::Deref for MainEntity {
    type Target = Entity;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub struct SyncAppPlugin<A: AppTag + Default + Clone>(A);
impl<A: AppTag + Default + Clone> SyncAppPlugin<A> {
    pub fn new() -> Self {
        Self(A::default())
    }

    fn sync_despawned_entities(
        despawned: Main<EventReader<Despawned>>,
        map: &mut EntityWorldMap,
        mut commands: Commands,
    ) {
        for despawned in despawned.into_inner() {
            if let Some(entity) = map.0.remove(&despawned.0) {
                commands.add(Despawn(entity));
            }
        }
    }
}

impl<A: AppTag + Default + Clone> Plugin for SyncAppPlugin<A> {
    fn setup(&mut self, app: &mut super::AppBuilder) {
        app.sub_app_mut(self.0.clone())
            .add_resource(EntityWorldMap::default())
            .add_systems(Extract, Self::sync_despawned_entities);
    }
}

pub struct SyncComponentPlugin<C: Component + Clone, A: AppTag>(std::marker::PhantomData<(C, A)>);
impl<C: Component + Clone, A: AppTag + Default + Clone> SyncComponentPlugin<C, A> {
    pub fn new() -> Self {
        Self(Default::default())
    }

    fn sync_component(
        query: Main<SQuery<(Entity, &C), Or<(Added<C>, Modified<C>)>>>,
        mut commands: Commands,
    ) {
        for (entity, component) in query.iter() {
            let component = component.clone();
            commands.add(move |world: &mut crate::World| {
                let sub_entity = unsafe {
                    let mut world = world.cell();
                    let map = world.get_mut().resource_mut::<EntityWorldMap>();
                    *map.0.entry(entity).or_insert_with(|| {
                        let entity = world.get_mut().spawn();
                        world.get_mut().add_component(entity, MainEntity(entity));
                        entity
                    })
                };

                world.add_component(sub_entity, component);
            });
        }
    }

    fn sync_removed_component(
        query: Main<SQuery<Entity, Removed<C>>>,
        map: &EntityWorldMap,
        mut commands: Commands,
    ) {
        for entity in query.iter() {
            if let Some(sub_entity) = map.0.get(&entity).copied() {
                commands.add(move |world: &mut crate::World| {
                    world.remove_component::<C>(sub_entity);
                });
            }
        }
    }
}

impl<C: Component + Clone, A: AppTag + Default + Clone> Plugin for SyncComponentPlugin<C, A> {
    fn setup(&mut self, app: &mut super::AppBuilder) {
        app.add_plugins(SyncAppPlugin::<A>::new())
            .sub_app_mut(A::default())
            .register::<C>()
            .register::<MainEntity>()
            .add_systems(Extract, Self::sync_component)
            .add_systems(Extract, Self::sync_removed_component);
    }
}
