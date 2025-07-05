use crate::{
    AppTag, Commands, Component, Despawn, Despawned, Entity, EventReader, Extract, Plugin,
    app::Main, system::Removed, unlifetime::SQuery,
};
use derive_ecs::Resource;
use std::collections::HashMap;

/// A resource that maps entities in the main world to entities in the sub world.
/// This is used to synchronize components between the two worlds.
#[derive(Resource, Default)]
pub struct EntityWorldMap(HashMap<Entity, Entity>);

pub struct SyncAppPlugin<A: AppTag + Clone>(A);
impl<A: AppTag + Clone> SyncAppPlugin<A> {
    pub fn new(value: A) -> Self {
        Self(value)
    }

    fn sync_despawned_entities(
        despawned: Main<EventReader<Despawned>>,
        map: &EntityWorldMap,
        mut commands: Commands,
    ) {
        for despawned in despawned.into_inner() {
            if map.0.contains_key(&despawned.0) {
                commands.add(Despawn(despawned.0));
            }
        }
    }
}

impl<A: AppTag + Clone> Plugin for SyncAppPlugin<A> {
    fn setup(&mut self, app: &mut super::AppBuilder) {
        app.add_sub_app(self.0.clone())
            .add_systems(Extract, Self::sync_despawned_entities);
    }
}

pub struct SyncComponentPlugin<C: Component + Clone>(std::marker::PhantomData<C>);
impl<C: Component + Clone> SyncComponentPlugin<C> {
    pub fn new() -> Self {
        Self(Default::default())
    }

    fn sync_component(query: Main<SQuery<(Entity, &C)>>, mut commands: Commands) {
        for (entity, component) in query.iter() {
            let component = component.clone();
            commands.add(move |world: &mut crate::World| {
                let sub_entity = unsafe {
                    let mut world = world.cell();
                    let map = world.get_mut().resource_mut::<EntityWorldMap>();
                    *map.0
                        .entry(entity)
                        .or_insert_with(|| world.get_mut().spawn())
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

impl<C: Component + Clone> Plugin for SyncComponentPlugin<C> {
    fn setup(&mut self, app: &mut super::AppBuilder) {
        app.register::<C>()
            .add_systems(Extract, Self::sync_component)
            .add_systems(Extract, Self::sync_removed_component);
    }
}
