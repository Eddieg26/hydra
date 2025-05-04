use super::CommandBuffer;
use crate::{
    Children, Command, Component, Entity, EntityCommands, EntityMut, Events, Parent, Removed,
    SystemArg, World, WorldCell,
};

pub struct Spawner<'world, 'state> {
    world: &'world mut World,
    commands: &'state mut CommandBuffer,
}

impl<'world, 'state> Spawner<'world, 'state> {
    pub fn new(world: &'world mut World, commands: &'state mut CommandBuffer) -> Self {
        Spawner { world, commands }
    }

    pub fn spawn(&'state mut self) -> EntityCommands<'state> {
        EntityCommands::new(self.world.entities.spawn(), self.commands)
    }

    pub fn spawn_with_parent(&'state mut self, parent: Entity) -> EntityCommands<'state> {
        let mut commands = EntityCommands::new(self.world.entities.spawn(), self.commands);
        commands.set_parent(parent);
        commands
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
            let Some(row) = world.despawn(entity) else {
                continue;
            };

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
    }
}

pub struct AddComponent<C: Component> {
    entity: Entity,
    component: C,
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

pub struct RemoveComponent<C: Component>(Entity, std::marker::PhantomData<C>);
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
                let event = removed.read.entities.get(&self.0).unwrap().last().unwrap();
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
        self.commands.add(AddComponent {
            entity: self.entity,
            component,
        });
    }

    pub fn remove_component<C: Component>(&mut self) {
        self.commands.add(RemoveComponent::<C>::new(self.entity));
    }

    pub fn set_parent(&mut self, parent: Entity) {
        self.commands.add(AddChild {
            parent,
            child: self.entity,
        });
    }

    pub fn remove_parent(&mut self) {
        self.commands.add(RemoveParent(self.entity));
    }

    pub fn add_child(&mut self, child: Entity) {
        self.commands.add(AddChild {
            parent: self.entity,
            child,
        });
    }

    pub fn add_children(&mut self, children: Vec<Entity>) {
        self.commands.add(AddChildren {
            parent: self.entity,
            children,
        });
    }

    pub fn insert_children(&mut self, index: usize, children: Vec<Entity>) {
        self.commands.add(InsertChildren {
            parent: self.entity,
            index,
            children,
        });
    }

    pub fn remove_child(&mut self, child: Entity) {
        self.commands.add(RemoveChild {
            parent: self.entity,
            child,
        });
    }

    pub fn remove_children(&mut self, children: Vec<Entity>) {
        self.commands.add(RemoveChildren {
            parent: self.entity,
            children,
        });
    }

    pub fn remove_all_children(&mut self) {
        self.commands.add(RemoveAllChildren(self.entity));
    }

    pub fn despawn(self) {
        self.commands.add(Despawn(self.entity));
    }
}

// impl<Q: BaseQuery, F: BaseFilter> ComponentQuery for Parent<Q, F> {
//     type Component = Parent;
//     type Query = Q;
//     type Filter = F;

//     type Item<'w> = Option<Q::Item<'w>>;

//     fn get<'w>(
//         state: &mut crate::SubQueryState<'w, Self::Query, Self::Filter>,
//         _: Entity,
//         row: crate::RowIndex,
//     ) -> Self::Item<'w> {
//         let SubQueryState {
//             query,
//             world,
//             column,
//             state,
//             filter,
//         } = state;

//         let Some(parent) = column
//             .and_then(|c| c.get::<Parent>(row.to_usize()))
//             .copied()
//         else {
//             return None;
//         };

//         let world = unsafe { world.get() };

//         let archetype = world.archetypes().entity_archetype(*parent)?;
//         let row = archetype.table().get_entity_row(*parent)?;

//         match F::filter(filter, *parent, row) && archetype.matches(&query) {
//             true => Some(Q::get(state, *parent, row)),
//             false => None,
//         }
//     }
// }

// impl<Q: BaseQuery, F: BaseFilter> ComponentQuery for Children<Q, F> {
//     type Component = Children;
//     type Query = Q;
//     type Filter = F;

//     type Item<'w> = Vec<Q::Item<'w>>;

//     fn get<'w>(
//         state: &mut crate::SubQueryState<'w, Self::Query, Self::Filter>,
//         _: Entity,
//         row: crate::RowIndex,
//     ) -> Self::Item<'w> {
//         let SubQueryState {
//             query,
//             world,
//             column,
//             state,
//             filter,
//         } = state;

//         let Some(children) = column.and_then(|c| c.get::<Children>(row.to_usize())) else {
//             return vec![];
//         };

//         let world = unsafe { world.get() };

//         children
//             .0
//             .iter()
//             .filter_map(|child| {
//                 let archetype = world.archetypes().entity_archetype(*child)?;
//                 let row = archetype.table().get_entity_row(*child)?;

//                 match F::filter(filter, *child, row) && archetype.matches(&query) {
//                     true => Some(Q::get(state, *child, row)),
//                     false => None,
//                 }
//             })
//             .collect()
//     }
// }

#[allow(unused_imports, dead_code)]
mod tests {
    use super::{
        AddChild, AddChildren, Despawn, RemoveAllChildren, RemoveChild, RemoveChildren, Spawner,
    };
    use crate::{Children, Command, Component, Parent, SystemArg, World};

    //     struct Age(u32);
    //     impl Component for Age {}

    //     #[test]
    //     fn test_spawn() {
    //         let mut world = World::new();
    //         world.register::<Age>();

    //         let entity = {
    //             let mut entities = vec![];
    //             let mut spawner = Spawner::new(&mut world, &mut entities);
    //             let entity = spawner.spawn().with_component(Age(0)).finish();

    //             Spawner::update(&mut entities, &mut world);

    //             entity
    //         };

    //         assert!(world.has_component::<Age>(entity))
    //     }

    //     #[test]
    //     fn test_spawn_with_parent() {
    //         let mut world = World::new();
    //         world.register::<Parent>();
    //         world.register::<Children>();

    //         let (parent, child) = {
    //             let mut entities = vec![];
    //             let mut spawner = Spawner::new(&mut world, &mut entities);
    //             let mut parent = spawner.spawn();
    //             let child = parent.add_child().finish();
    //             let parent = parent.finish();

    //             Spawner::update(&mut entities, &mut world);

    //             (parent, child)
    //         };

    //         let parent_component = world.get_component::<Parent>(child).unwrap();
    //         assert_eq!(parent_component, &Parent::from(parent));

    //         let children = world.get_component::<Children>(parent).unwrap();
    //         assert!(children.as_slice().iter().any(|c| *c == child));
    //     }

    //     #[test]
    //     fn test_despawn() {
    //         let mut world = World::new();
    //         world.register::<Parent>();
    //         world.register::<Children>();

    //         let (parent, child) = {
    //             let mut entities = vec![];
    //             let mut spawner = Spawner::new(&mut world, &mut entities);
    //             let mut parent = spawner.spawn();
    //             let child = parent.add_child().finish();
    //             let parent = parent.finish();

    //             Spawner::update(&mut entities, &mut world);

    //             (parent, child)
    //         };

    //         let parent_component = world.get_component::<Parent>(child).unwrap();
    //         assert_eq!(parent_component, &Parent::from(parent));

    //         let children = world.get_component::<Children>(parent).unwrap();
    //         assert!(children.as_slice().iter().any(|c| *c == child));

    //         Despawn(parent).execute(&mut world);

    //         assert!(world.archetypes.get_entity(parent).is_none());
    //         assert!(world.archetypes.get_entity(child).is_none());
    //     }

    //     #[test]
    //     fn add_child() {
    //         let mut world = World::new();
    //         world.register::<Parent>();
    //         world.register::<Children>();

    //         let parent = world.spawn();
    //         let child = world.spawn();

    //         AddChild { parent, child }.execute(&mut world);

    //         let child_parent = world.get_component::<Parent>(child).copied().unwrap();
    //         let children = world.get_component::<Children>(parent).unwrap();

    //         assert!(children.0.contains(&child));
    //         assert_eq!(*child_parent, parent);
    //     }

    //     #[test]
    //     fn add_children() {
    //         let mut world = World::new();
    //         world.register::<Parent>();
    //         world.register::<Children>();

    //         let parent = world.spawn();
    //         let entities = (0..3).map(|_| world.spawn()).collect::<Vec<_>>();

    //         AddChildren {
    //             parent,
    //             children: entities.clone(),
    //         }
    //         .execute(&mut world);

    //         let children_a = world.get_component::<Children>(parent).unwrap();
    //         assert!(children_a.0 == entities);
    //         for child in entities {
    //             let child_parent = world.get_component::<Parent>(child).copied().unwrap();
    //             assert_eq!(*child_parent, parent);
    //         }
    //     }

    //     #[test]
    //     fn remove_child() {
    //         let mut world = World::new();
    //         world.register::<Parent>();
    //         world.register::<Children>();

    //         let parent = world.spawn();
    //         let child = world.spawn();

    //         AddChild { parent, child }.execute(&mut world);

    //         RemoveChild { parent, child }.execute(&mut world);

    //         let child_parent = world.get_component::<Parent>(child);
    //         let children = world.get_component::<Children>(parent);

    //         assert!(child_parent.is_none());
    //         assert!(children.is_none());
    //     }

    //     #[test]
    //     fn remove_children() {
    //         let mut world = World::new();
    //         world.register::<Parent>();
    //         world.register::<Children>();

    //         let parent = world.spawn();
    //         let entities = (0..3).map(|_| world.spawn()).collect::<Vec<_>>();

    //         AddChildren {
    //             parent,
    //             children: entities.clone(),
    //         }
    //         .execute(&mut world);

    //         RemoveChildren {
    //             parent,
    //             children: entities.clone(),
    //         }
    //         .execute(&mut world);

    //         let children = world.get_component::<Children>(parent);

    //         assert!(children.is_none());
    //         for child in entities {
    //             let parent = world.get_component::<Parent>(child);
    //             assert!(parent.is_none());
    //         }
    //     }

    //     #[test]
    //     fn remove_all_children() {
    //         let mut world = World::new();
    //         world.register::<Parent>();
    //         world.register::<Children>();

    //         let parent = world.spawn();
    //         let entities = (0..3).map(|_| world.spawn()).collect::<Vec<_>>();

    //         AddChildren {
    //             parent,
    //             children: entities.clone(),
    //         }
    //         .execute(&mut world);

    //         RemoveAllChildren(parent).execute(&mut world);

    //         let children = world.get_component::<Children>(parent);

    //         assert!(children.is_none());
    //         for child in entities {
    //             let parent = world.get_component::<Parent>(child);
    //             assert!(parent.is_none());
    //         }
    //     }
    // }

    // #[allow(unused_imports)]
    // mod tests {
    //     use super::{
    //         AddChild, AddChildren, Children, Parent, RemoveAllChildren, RemoveChild, RemoveChildren,
    //     };
    //     use crate::{Command, World};

    //     #[test]
    //     fn add_child() {
    //         let mut world = World::new();
    //         world.register::<Parent>();
    //         world.register::<Children>();

    //         let parent = world.spawn();
    //         let child = world.spawn();

    //         AddChild { parent, child }.execute(&mut world);

    //         let child_parent = world.get_component::<Parent>(child).copied().unwrap();
    //         let children = world.get_component::<Children>(parent).unwrap();

    //         assert!(children.0.contains(&child));
    //         assert_eq!(*child_parent, parent);
    //     }

    //     #[test]
    //     fn add_children() {
    //         let mut world = World::new();
    //         world.register::<Parent>();
    //         world.register::<Children>();

    //         let parent = world.spawn();
    //         let entities = (0..3).map(|_| world.spawn()).collect::<Vec<_>>();

    //         AddChildren {
    //             parent,
    //             children: entities.clone(),
    //         }
    //         .execute(&mut world);

    //         let children_a = world.get_component::<Children>(parent).unwrap();
    //         assert!(children_a.0 == entities);
    //         for child in entities {
    //             let child_parent = world.get_component::<Parent>(child).copied().unwrap();
    //             assert_eq!(*child_parent, parent);
    //         }
    //     }

    //     #[test]
    //     fn remove_child() {
    //         let mut world = World::new();
    //         world.register::<Parent>();
    //         world.register::<Children>();

    //         let parent = world.spawn();
    //         let child = world.spawn();

    //         AddChild { parent, child }.execute(&mut world);

    //         RemoveChild { parent, child }.execute(&mut world);

    //         let child_parent = world.get_component::<Parent>(child);
    //         let children = world.get_component::<Children>(parent);

    //         assert!(child_parent.is_none());
    //         assert!(children.is_none());
    //     }

    //     #[test]
    //     fn remove_children() {
    //         let mut world = World::new();
    //         world.register::<Parent>();
    //         world.register::<Children>();

    //         let parent = world.spawn();
    //         let entities = (0..3).map(|_| world.spawn()).collect::<Vec<_>>();

    //         AddChildren {
    //             parent,
    //             children: entities.clone(),
    //         }
    //         .execute(&mut world);

    //         RemoveChildren {
    //             parent,
    //             children: entities.clone(),
    //         }
    //         .execute(&mut world);

    //         let children = world.get_component::<Children>(parent);

    //         assert!(children.is_none());
    //         for child in entities {
    //             let parent = world.get_component::<Parent>(child);
    //             assert!(parent.is_none());
    //         }
    //     }

    //     #[test]
    //     fn remove_all_children() {
    //         let mut world = World::new();
    //         world.register::<Parent>();
    //         world.register::<Children>();

    //         let parent = world.spawn();
    //         let entities = (0..3).map(|_| world.spawn()).collect::<Vec<_>>();

    //         AddChildren {
    //             parent,
    //             children: entities.clone(),
    //         }
    //         .execute(&mut world);

    //         RemoveAllChildren(parent).execute(&mut world);

    //         let children = world.get_component::<Children>(parent);

    //         assert!(children.is_none());
    //         for child in entities {
    //             let parent = world.get_component::<Parent>(child);
    //             assert!(parent.is_none());
    //         }
    //     }
}
