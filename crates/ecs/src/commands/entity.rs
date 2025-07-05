use super::CommandBuffer;
use crate::{
    Children, Command, Component, ComponentKit, Entity, EntityCommands, EntityMut, Events, Parent,
    SystemArg, World, system::Removed, world::WorldCell,
};
use derive_ecs::Event;

pub struct Spawner<'world, 'state> {
    world: &'world mut World,
    commands: &'state mut CommandBuffer,
}

impl<'world, 'state> Spawner<'world, 'state> {
    pub fn new(world: &'world mut World, commands: &'state mut CommandBuffer) -> Self {
        Spawner { world, commands }
    }

    pub fn spawn(&mut self) -> Spawned<'_> {
        let mut commands = EntityCommands::new(self.world.entities.spawn(), self.commands);
        commands.add(|entity, world: &mut World| {
            world.archetypes.add_entity(entity);
        });
        Spawned { commands }
    }

    pub fn spawn_with_parent(&mut self, parent: Entity) -> Spawned<'_> {
        let mut commands = EntityCommands::new(self.world.entities.spawn(), self.commands);
        commands.add(|entity, world: &mut World| {
            world.archetypes.add_entity(entity);
        });
        commands.set_parent(parent);
        Spawned { commands }
    }
}

unsafe impl SystemArg for Spawner<'_, '_> {
    type Item<'world, 'state> = Spawner<'world, 'state>;

    type State = CommandBuffer;

    fn init(_: &mut World, _: &mut crate::WorldAccess) -> Self::State {
        CommandBuffer::new()
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

    fn update(state: &mut Self::State, world: &mut World) {
        state.execute(world);
    }
}

pub struct Spawned<'s> {
    commands: EntityCommands<'s>,
}

impl<'s> Spawned<'s> {
    pub fn entity(&self) -> Entity {
        self.commands.entity
    }

    pub fn add_component<C: Component>(&mut self, component: C) {
        self.commands.add_component(component);
    }

    pub fn add_components<C: ComponentKit>(&mut self, components: C) {
        self.commands.add_components(components);
    }

    pub fn set_parent(&mut self, parent: Entity) {
        self.commands.set_parent(parent);
    }

    pub fn add_child(&mut self, child: Entity) {
        self.commands.add_child(child);
    }

    pub fn add_children(&mut self, children: Vec<Entity>) {
        self.commands.add_children(children);
    }

    pub fn with_component<C: Component>(mut self, component: C) -> Self {
        self.commands.add_component(component);
        self
    }

    pub fn with_components<C: ComponentKit>(mut self, components: C) -> Self {
        self.commands.add_components(components);
        self
    }

    pub fn with_parent(mut self, parent: Entity) -> Self {
        self.commands.set_parent(parent);
        self
    }

    pub fn with_child(mut self, child: Entity) -> Self {
        self.add_child(child);
        self
    }

    pub fn with_children(mut self, children: Vec<Entity>) -> Self {
        self.commands.add_children(children);
        self
    }

    pub fn finish(self) -> Entity {
        self.commands.finish()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Spawn;
impl Command for Spawn {
    fn execute(self, world: &mut World) {
        world.spawn();
    }
}

#[derive(Event)]
pub struct Despawned(pub Entity);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Despawn(pub Entity);

impl Command for Despawn {
    fn execute(self, world: &mut World) {
        let id = world.components().get_id::<Children>().unwrap();
        let parent = world.get_component::<Parent>(self.0).copied();

        let mut stack = vec![self.0];
        let mut despawned = Vec::new();
        while let Some(entity) = stack.pop() {
            let Some(row) = world.despawn(entity) else {
                continue;
            };

            despawned.push(Despawned(entity));

            let Some(children) = row.get::<Children>(id) else {
                continue;
            };

            for child in children.as_slice() {
                stack.push(*child);
            }
        }

        if let Some(children) =
            parent.and_then(|parent| world.get_component_mut::<Children>(*parent))
        {
            children.retain(|child| *child != self.0);
        }

        let mut events = world.resource_mut::<Events<Despawned>>().writer();
        events.send_batch(despawned);
    }
}

pub struct AddComponent<C: Component> {
    pub entity: Entity,
    pub component: C,
}

impl<C: Component> AddComponent<C> {
    pub fn new(entity: Entity, component: C) -> Self {
        Self { entity, component }
    }
}

impl<C: Component> Command for AddComponent<C> {
    fn execute(self, world: &mut World) {
        world.add_component(self.entity, self.component);
    }
}

pub struct AddComponents<C: ComponentKit> {
    pub entity: Entity,
    pub components: C,
}

impl<C: ComponentKit> AddComponents<C> {
    pub fn new(entity: Entity, components: C) -> Self {
        Self { entity, components }
    }
}

impl<C: ComponentKit> Command for AddComponents<C> {
    fn execute(self, world: &mut World) {
        world.add_components(self.entity, self.components);
    }
}

pub struct RemoveComponent<C: Component>(pub Entity, std::marker::PhantomData<C>);
impl<C: Component> RemoveComponent<C> {
    pub fn new(entity: Entity) -> Self {
        Self(entity, Default::default())
    }

    pub fn entity(&self) -> Entity {
        self.0
    }
}

impl<C: Component> Command for RemoveComponent<C> {
    fn execute(self, world: &mut World) {
        world.remove_component::<C>(self.0);
    }
}

pub struct AddChild {
    pub parent: Entity,
    pub child: Entity,
}

impl AddChild {
    pub fn new(parent: Entity, child: Entity) -> Self {
        Self { parent, child }
    }
}

impl Command for AddChild {
    fn execute(self, world: &mut super::World) {
        if let Some(parent) = world.get_component::<Parent>(self.child).copied() {
            EntityMut::remove_child(world, *parent, self.child);
        };

        let mut parent = world.entity_mut(self.parent);

        if let Some(children) = parent.get_component_mut::<Children>() {
            children.push(self.child);
        } else {
            let children = Children::from(self.child);
            parent.add_component::<Children>(children);
        }

        world.add_component::<Parent>(self.child, Parent::from(self.parent));
    }
}

pub struct AddChildren {
    pub parent: Entity,
    pub children: Vec<Entity>,
}

impl AddChildren {
    pub fn new(parent: Entity, children: Vec<Entity>) -> Self {
        Self { parent, children }
    }
}

impl Command for AddChildren {
    fn execute(self, world: &mut World) {
        let mut parent = world.entity_mut(self.parent);
        parent.add_children(self.children);
    }
}

pub struct InsertChildren {
    pub parent: Entity,
    pub children: Vec<Entity>,
    pub index: usize,
}

impl InsertChildren {
    pub fn new(parent: Entity, children: Vec<Entity>, index: usize) -> Self {
        Self {
            parent,
            children,
            index,
        }
    }
}

impl Command for InsertChildren {
    fn execute(self, world: &mut World) {
        let mut parent = world.entity_mut(self.parent);
        parent.insert_children(self.index, self.children);
    }
}

pub struct RemoveChild {
    pub parent: Entity,
    pub child: Entity,
}

impl RemoveChild {
    pub fn new(parent: Entity, child: Entity) -> Self {
        Self { parent, child }
    }
}

impl Command for RemoveChild {
    fn execute(self, world: &mut World) {
        if EntityMut::remove_child(world, self.parent, self.child) {
            world.remove_component::<Parent>(self.child);
        }
    }
}

pub struct RemoveChildren {
    pub parent: Entity,
    pub children: Vec<Entity>,
}

impl RemoveChildren {
    pub fn new(parent: Entity, children: Vec<Entity>) -> Self {
        Self { parent, children }
    }
}

impl Command for RemoveChildren {
    fn execute(self, world: &mut World) {
        for removed in EntityMut::remove_children(world, self.parent, self.children) {
            world.remove_component::<Parent>(removed);
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct RemoveAllChildren(pub Entity);
impl Command for RemoveAllChildren {
    fn execute(self, world: &mut World) {
        if let Some(_) = world.remove_component::<Children>(self.0) {
            let children = {
                let removed = world.resource::<Events<Removed<Children>>>();
                let event = removed.write.entities.get(&self.0).unwrap().last().unwrap();
                removed.write.events[*event].clone()
            };

            for child in children.as_slice() {
                world.remove_component::<Parent>(*child);
            }
        }
    }
}

pub struct SetParent {
    pub parent: Entity,
    pub entity: Entity,
}

impl Command for SetParent {
    fn execute(self, world: &mut World) {
        AddChild {
            parent: self.parent,
            child: self.entity,
        }
        .execute(world);
    }
}

pub struct RemoveParent(Entity);

impl Command for RemoveParent {
    fn execute(self, world: &mut World) {
        if let Some(parent) = world.get_component::<Parent>(self.0).copied() {
            if let Some(children) = world.get_component_mut::<Children>(*parent) {
                children.retain(|child| *child != self.0);
            }

            world.remove_component::<Parent>(self.0);
        }
    }
}

impl<'w> EntityMut<'w> {
    pub fn add_children(&mut self, entities: Vec<Entity>) {
        self.insert_children(0, entities);
    }

    pub fn insert_children(&mut self, index: usize, entities: Vec<Entity>) {
        Self::update_parents(self.world, self.id(), &entities);

        match self.get_component_mut::<Children>() {
            Some(children) => children.insert(index, entities),
            None => {
                self.add_component(Children::from(entities));
            }
        };
    }

    fn remove_child(world: &mut World, parent: Entity, child: Entity) -> bool {
        let mut parent = world.entity_mut(parent);

        if let Some(children) = parent.get_component_mut::<Children>() {
            let removed = children.remove(child);

            if removed && children.is_empty() {
                parent.remove_component::<Children>();
            }

            removed
        } else {
            false
        }
    }

    fn remove_children(world: &mut World, parent: Entity, entities: Vec<Entity>) -> Vec<Entity> {
        let mut parent = world.entity_mut(parent);
        let mut removed = vec![];
        if let Some(children) = parent.get_component_mut::<Children>() {
            children.retain(|child| match entities.contains(child) {
                true => {
                    removed.push(*child);
                    false
                }
                false => true,
            });

            if children.is_empty() {
                parent.remove_component::<Children>();
            }
        }

        removed
    }

    fn update_parent(world: &mut World, parent: Entity, child: Entity) -> Option<Entity> {
        if parent == child {
            panic!("Not allowed to set entity as a child of itself.")
        }

        let mut child = world.entity_mut(child);
        if let Some(current) = child.get_component_mut::<Parent>() {
            Some(current.set(parent))
        } else {
            child.add_component(Parent::from(parent));
            None
        }
    }

    fn update_parents(world: &mut World, parent: Entity, children: &[Entity]) {
        for child in children {
            if let Some(previous) = Self::update_parent(world, parent, *child) {
                if previous != parent {
                    continue;
                }

                Self::remove_child(world, previous, *child);
            }
        }
    }
}

impl EntityCommands<'_> {
    pub fn add_component<C: Component>(&mut self, component: C) {
        self.buffer.add(AddComponent::new(self.entity, component));
    }

    pub fn add_components<C: ComponentKit>(&mut self, components: C) {
        self.buffer.add(AddComponents::new(self.entity, components));
    }

    pub fn remove_component<C: Component>(&mut self) {
        self.buffer.add(RemoveComponent::<C>::new(self.entity));
    }

    pub fn set_parent(&mut self, parent: Entity) {
        self.buffer.add(AddChild::new(parent, self.entity));
    }

    pub fn remove_parent(&mut self) {
        self.buffer.add(RemoveParent(self.entity));
    }

    pub fn add_child(&mut self, child: Entity) {
        self.buffer.add(AddChild::new(self.entity, child));
    }

    pub fn add_children(&mut self, children: Vec<Entity>) {
        self.buffer.add(AddChildren::new(self.entity, children));
    }

    pub fn insert_children(&mut self, index: usize, children: Vec<Entity>) {
        self.buffer
            .add(InsertChildren::new(self.entity, children, index));
    }

    pub fn remove_child(&mut self, child: Entity) {
        self.buffer.add(RemoveChild::new(self.entity, child));
    }

    pub fn remove_children(&mut self, children: Vec<Entity>) {
        self.buffer.add(RemoveChildren::new(self.entity, children));
    }

    pub fn remove_all_children(&mut self) {
        self.buffer.add(RemoveAllChildren(self.entity));
    }

    pub fn despawn(self) {
        self.buffer.add(Despawn(self.entity));
    }
}

#[allow(unused_imports, dead_code)]
mod tests {
    use super::{
        AddChild, AddChildren, Despawn, RemoveAllChildren, RemoveChild, RemoveChildren, Spawner,
    };
    use crate::{Children, Command, CommandBuffer, Component, Parent, SystemArg, World};

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    struct Age(u32);
    impl Component for Age {}

    #[test]
    fn spawn() {
        let mut world = World::new();
        world.register::<Age>();

        let mut commands = CommandBuffer::new();
        let mut spawner = Spawner::new(&mut world, &mut commands);
        let entity = spawner.spawn().with_component(Age(0)).finish();

        Spawner::update(&mut commands, &mut world);

        assert!(world.has_component::<Age>(entity))
    }

    #[test]
    fn spawn_with_parent() {
        let mut world = World::new();
        world.register::<Parent>();
        world.register::<Children>();

        let mut commands = CommandBuffer::new();
        let mut spawner = Spawner::new(&mut world, &mut commands);
        let parent = spawner.spawn().finish();
        let child = spawner.spawn().with_parent(parent).finish();

        Spawner::update(&mut commands, &mut world);

        let child_parent = world.get_component::<Parent>(child).copied().unwrap();
        assert_eq!(child_parent, Parent::from(parent));

        let children = world.get_component::<Children>(parent).unwrap();
        assert!(children.contains(child));
    }

    #[test]
    fn despawn() {
        let mut world = World::new();

        let mut commands = CommandBuffer::new();
        let mut spawner = Spawner::new(&mut world, &mut commands);
        let entity = spawner.spawn().finish();

        Spawner::update(&mut commands, &mut world);

        assert!(world.archetypes().get_entity(entity).is_some());

        world.despawn(entity);

        assert!(world.archetypes().get_entity(entity).is_none());
    }

    #[test]
    fn despawn_hierarchy() {
        let mut world = World::new();
        world.register::<Parent>();
        world.register::<Children>();

        let mut commands = CommandBuffer::new();
        let mut spawner = Spawner::new(&mut world, &mut commands);
        let parent = spawner.spawn().finish();
        let child = spawner.spawn().with_parent(parent).finish();

        Spawner::update(&mut commands, &mut world);

        Despawn(parent).execute(&mut world);

        assert!(world.archetypes.get_entity(parent).is_none());
        assert!(world.archetypes.get_entity(child).is_none());
    }

    #[test]
    fn add_child() {
        let mut world = World::new();
        world.register::<Parent>();
        world.register::<Children>();

        let parent = world.spawn();
        let child = world.spawn();

        AddChild::new(parent, child).execute(&mut world);

        let child_parent = world.get_component::<Parent>(child).copied().unwrap();
        let children = world.get_component::<Children>(parent).unwrap();

        assert!(children.contains(child));
        assert_eq!(child_parent, Parent::from(parent));
    }

    #[test]
    fn add_children() {
        let mut world = World::new();
        world.register::<Parent>();
        world.register::<Children>();

        let parent = world.spawn();
        let entities = (0..3).map(|_| world.spawn()).collect::<Vec<_>>();

        AddChildren::new(parent, entities.clone()).execute(&mut world);

        let children = world.get_component::<Children>(parent).unwrap();
        assert!(children.as_slice() == entities.as_slice());
        for child in entities {
            let child_parent = world.get_component::<Parent>(child).copied().unwrap();
            assert_eq!(child_parent, Parent::from(parent));
        }
    }

    #[test]
    fn remove_child() {
        let mut world = World::new();
        world.register::<Parent>();
        world.register::<Children>();

        let parent = world.spawn();
        let child = world.spawn();

        AddChild::new(parent, child).execute(&mut world);

        RemoveChild::new(parent, child).execute(&mut world);

        let child_parent = world.get_component::<Parent>(child);
        let children = world.get_component::<Children>(parent);

        assert!(child_parent.is_none());
        assert!(children.is_none());
    }

    #[test]
    fn remove_children() {
        let mut world = World::new();
        world.register::<Parent>();
        world.register::<Children>();

        let parent = world.spawn();
        let entities = (0..3).map(|_| world.spawn()).collect::<Vec<_>>();

        AddChildren::new(parent, entities.clone()).execute(&mut world);

        let removed = entities[1..].iter().copied().collect::<Vec<_>>();
        RemoveChildren::new(parent, removed.clone()).execute(&mut world);

        let children = world.get_component::<Children>(parent).unwrap();
        assert!(children.len() == 1);
        assert_eq!(children[0], entities[0]);

        let child_parent = world.get_component::<Parent>(children[0]).copied().unwrap();
        assert_eq!(child_parent, Parent::from(parent));

        for child in removed {
            let child_parent = world.get_component::<Parent>(child);
            assert!(child_parent.is_none());
        }
    }

    #[test]
    fn remove_all_children() {
        let mut world = World::new();
        world.register::<Parent>();
        world.register::<Children>();

        let parent = world.spawn();
        let entities = (0..3).map(|_| world.spawn()).collect::<Vec<_>>();

        AddChildren::new(parent, entities.clone()).execute(&mut world);

        RemoveAllChildren(parent).execute(&mut world);

        let children = world.get_component::<Children>(parent);

        assert!(children.is_none());
        for child in entities {
            let parent = world.get_component::<Parent>(child);
            assert!(parent.is_none());
        }
    }
}
