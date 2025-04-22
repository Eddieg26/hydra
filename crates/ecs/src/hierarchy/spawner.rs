use super::{Children, Command, Parent};
use crate::{Component, Entity, EntityCommands, Row, SystemArg, SystemInit, World, WorldCell};

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
        let id = self.world.entities_mut().spawn();
        Spawned {
            id,
            parent: None,
            children: vec![],
            components: Row::new(),
            spawner: self,
        }
    }

    pub fn spawn_with_parent(&mut self, parent: Entity) -> Spawned<'world, 'state, '_> {
        let id = self.world.entities_mut().spawn();

        Spawned {
            id,
            parent: Some(parent),
            children: vec![],
            components: Row::new(),
            spawner: self,
        }
    }
}

pub struct Spawned<'world, 'state, 'spawner> {
    id: Entity,
    components: Row,
    parent: Option<Entity>,
    children: Vec<Entity>,
    spawner: &'spawner mut Spawner<'world, 'state>,
}

impl<'world, 'state, 'spawner> Spawned<'world, 'state, 'spawner> {
    pub fn add_component<C: Component>(&mut self, component: C) -> &mut Self {
        let id = unsafe { self.spawner.world.components().get_id_unchecked::<C>() };
        self.components.insert(id, component);
        self
    }

    pub fn add_child(&mut self) -> Spawned<'world, 'state, '_> {
        let child = self.spawner.spawn_with_parent(self.id);
        self.children.push(child.id);

        child
    }

    pub fn finish(mut self) -> Entity {
        let entity = self.id;

        if let Some(parent) = self.parent {
            let id = unsafe { self.spawner.world.components().get_id_unchecked::<Parent>() };
            self.components
                .insert(id, Parent(parent, Default::default()));
        }

        if !self.children.is_empty() {
            let id = unsafe {
                self.spawner
                    .world
                    .components()
                    .get_id_unchecked::<Children>()
            };

            self.components
                .insert(id, Children(self.children, Default::default()));
        }

        self.spawner.entities.push((entity, self.components));

        entity
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Spawn;
impl Command for Spawn {
    fn execute(self, world: &mut World) {
        world.spawn();
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Despawn(pub Entity);

impl Command for Despawn {
    fn execute(self, world: &mut World) {
        let id = world.components().get_id::<Children>().unwrap();
        let parent = world.get_component::<Parent>(self.0).copied();

        let mut stack = vec![self.0];
        while let Some(entity) = stack.pop() {
            let Some((_, row)) = world.despawn(entity) else {
                continue;
            };

            let Some(children) = row.get::<Children>(id) else {
                continue;
            };

            for child in &children.0 {
                stack.push(*child);
            }
        }

        if let Some(children) =
            parent.and_then(|parent| world.get_component_mut::<Children>(*parent))
        {
            children.0.retain(|child| *child != self.0);
        }
    }
}

impl EntityCommands<'_> {
    pub fn despawn(self) {
        self.commands.add(Despawn(self.entity));
    }
}
