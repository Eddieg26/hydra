use crate::{Component, Entity, Row, SystemArg, SystemInit, World, WorldCell};

pub struct Spawner<'world, 'state> {
    world: &'world mut World,
    entities: &'state mut Vec<(Entity, Row)>,
    _marker: std::marker::PhantomData<&'state ()>,
}

impl<'world, 'state> Spawner<'world, 'state> {
    pub fn new(world: &'world mut World, entities: &'state mut Vec<(Entity, Row)>) -> Self {
        Spawner {
            world,
            entities,
            _marker: std::marker::PhantomData,
        }
    }

    pub fn spawn(&mut self) -> Spawned<'world, 'state, '_> {
        let id = self.world.spawn();
        Spawned {
            id,
            components: Row::new(),
            spawner: self,
        }
    }
}

unsafe impl SystemArg for Spawner<'_, '_> {
    type Item<'world, 'state> = Spawner<'world, 'state>;

    type State = Vec<(Entity, Row)>;

    fn init(_: &mut SystemInit) -> Self::State {
        vec![]
    }

    unsafe fn get<'world, 'state>(
        state: &'state mut Self::State,
        mut world: WorldCell<'world>,
        _: &crate::system::SystemMeta,
    ) -> Self::Item<'world, 'state> {
        unsafe { Spawner::new(world.get_mut(), state) }
    }

    fn exclusive() -> bool {
        true
    }

    fn apply(state: &mut Self::State, world: &mut World) {
        for (entity, components) in state.drain(..) {
            world.add_components(entity, components);
        }
    }
}

pub struct Spawned<'world, 'state, 'spawner> {
    id: Entity,
    components: Row,
    spawner: &'spawner mut Spawner<'world, 'state>,
}

impl<'world, 'state, 'spawner> Spawned<'world, 'state, 'spawner> {
    pub fn with<C: Component>(mut self, component: C) -> Self {
        let id = unsafe { self.spawner.world.components().get_id_unchecked::<C>() };
        self.components.insert(id, component);
        self
    }

    pub fn finish(self) -> Entity {
        let id = self.id;
        self.spawner.entities.push((id, self.components));
        id
    }
}
