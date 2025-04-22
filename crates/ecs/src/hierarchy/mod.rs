use super::{Command, Component, Entity, World};
use crate::{
    BaseFilter, BaseQuery, ComponentQuery, EntityCommands, EntityMut, SparseIndex, SubQueryState,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Parent<Q: BaseQuery = (), F: BaseFilter = ()>(Entity, std::marker::PhantomData<(Q, F)>);
impl Parent {
    pub fn new(entity: Entity) -> Self {
        Self(entity, Default::default())
    }

    pub fn get(&self) -> Entity {
        self.0
    }
}

impl std::ops::Deref for Parent {
    type Target = Entity;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Component for Parent {}

pub struct Children<Q: BaseQuery = (), F: BaseFilter = ()>(
    Vec<Entity>,
    std::marker::PhantomData<(Q, F)>,
);
impl Children {
    pub fn new() -> Self {
        Self(vec![], Default::default())
    }

    pub fn with_child(child: Entity) -> Self {
        Self(vec![child], Default::default())
    }

    pub fn get(&self) -> &[Entity] {
        &self.0
    }

    pub fn iter(&self) -> std::slice::Iter<'_, Entity> {
        self.0.iter()
    }
}

impl From<&[Entity]> for Children {
    fn from(value: &[Entity]) -> Self {
        Children(value.to_vec(), Default::default())
    }
}

impl From<Vec<Entity>> for Children {
    fn from(value: Vec<Entity>) -> Self {
        Children(value, Default::default())
    }
}

impl std::ops::Deref for Children {
    type Target = [Entity];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Component for Children {}

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
            children.0.push(self.child);
        } else {
            let children = Children::with_child(self.child);
            parent.add_component::<Children>(children);
        }

        world.add_component::<Parent>(self.child, Parent::new(self.parent));
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
        if let Some((_, children)) = world.remove_component::<Children>(self.0) {
            for child in children.0 {
                world.remove_component::<Parent>(child);
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
        if let Some((_, parent)) = world.remove_component::<Parent>(self.0) {
            if let Some(children) = world.get_component_mut::<Children>(*parent) {
                children.0.retain(|child| *child != self.0);
            }
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
            Some(children) => {
                children.0.splice(index..index, entities);
                children.0.dedup();
            }
            None => {
                self.add_component(Children::from(entities));
            }
        };
    }

    fn remove_child(world: &mut World, parent: Entity, child: Entity) -> bool {
        let mut parent = world.entity_mut(parent);

        if let Some(children) = parent.get_component_mut::<Children>() {
            let removed = children
                .iter()
                .position(|e| *e == child)
                .and_then(|i| Some(children.0.remove(i)))
                .is_some();

            if removed && children.0.is_empty() {
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
            children.0.retain(|child| match entities.contains(child) {
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
            let previous = current.get();
            current.0 = parent;
            Some(previous)
        } else {
            child.add_component(Parent::new(parent));
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

impl<Q: BaseQuery, F: BaseFilter> ComponentQuery for Parent<Q, F> {
    type Component = Parent;
    type Query = Q;
    type Filter = F;

    type Item<'w> = Option<Q::Item<'w>>;

    fn get<'w>(
        state: &mut crate::SubQueryState<'w, Self::Query, Self::Filter>,
        _: Entity,
        row: crate::RowIndex,
    ) -> Self::Item<'w> {
        let SubQueryState {
            query,
            world,
            column,
            state,
            filter,
        } = state;

        let Some(parent) = column
            .and_then(|c| c.get::<Parent>(row.to_usize()))
            .copied()
        else {
            return None;
        };

        let world = unsafe { world.get() };

        let archetype = world.archetypes().entity_archetype(*parent)?;
        let row = archetype.table().get_entity_row(*parent)?;

        match F::filter(filter, *parent, row) && archetype.matches(&query) {
            true => Some(Q::get(state, *parent, row)),
            false => None,
        }
    }
}

impl<Q: BaseQuery, F: BaseFilter> ComponentQuery for Children<Q, F> {
    type Component = Children;
    type Query = Q;
    type Filter = F;

    type Item<'w> = Vec<Q::Item<'w>>;

    fn get<'w>(
        state: &mut crate::SubQueryState<'w, Self::Query, Self::Filter>,
        _: Entity,
        row: crate::RowIndex,
    ) -> Self::Item<'w> {
        let SubQueryState {
            query,
            world,
            column,
            state,
            filter,
        } = state;

        let Some(children) = column.and_then(|c| c.get::<Children>(row.to_usize())) else {
            return vec![];
        };

        let world = unsafe { world.get() };

        children
            .0
            .iter()
            .filter_map(|child| {
                let archetype = world.archetypes().entity_archetype(*child)?;
                let row = archetype.table().get_entity_row(*child)?;

                match F::filter(filter, *child, row) && archetype.matches(&query) {
                    true => Some(Q::get(state, *child, row)),
                    false => None,
                }
            })
            .collect()
    }
}

impl EntityCommands<'_> {
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

#[allow(unused_imports)]
mod tests {
    use super::{
        AddChild, AddChildren, Children, Parent, RemoveAllChildren, RemoveChild, RemoveChildren,
    };
    use crate::{Command, World};

    #[test]
    fn add_child() {
        let mut world = World::new();
        world.register::<Parent>();
        world.register::<Children>();

        let parent = world.spawn();
        let child = world.spawn();

        AddChild { parent, child }.execute(&mut world);

        let child_parent = world.get_component::<Parent>(child).copied().unwrap();
        let children = world.get_component::<Children>(parent).unwrap();

        assert!(children.0.contains(&child));
        assert_eq!(*child_parent, parent);
    }

    #[test]
    fn add_children() {
        let mut world = World::new();
        world.register::<Parent>();
        world.register::<Children>();

        let parent = world.spawn();
        let entities = (0..3).map(|_| world.spawn()).collect::<Vec<_>>();

        AddChildren {
            parent,
            children: entities.clone(),
        }
        .execute(&mut world);

        let children_a = world.get_component::<Children>(parent).unwrap();
        assert!(children_a.0 == entities);
        for child in entities {
            let child_parent = world.get_component::<Parent>(child).copied().unwrap();
            assert_eq!(*child_parent, parent);
        }
    }

    #[test]
    fn remove_child() {
        let mut world = World::new();
        world.register::<Parent>();
        world.register::<Children>();

        let parent = world.spawn();
        let child = world.spawn();

        AddChild { parent, child }.execute(&mut world);

        RemoveChild { parent, child }.execute(&mut world);

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

        AddChildren {
            parent,
            children: entities.clone(),
        }
        .execute(&mut world);

        RemoveChildren {
            parent,
            children: entities.clone(),
        }
        .execute(&mut world);

        let children = world.get_component::<Children>(parent);

        assert!(children.is_none());
        for child in entities {
            let parent = world.get_component::<Parent>(child);
            assert!(parent.is_none());
        }
    }

    #[test]
    fn remove_all_children() {
        let mut world = World::new();
        world.register::<Parent>();
        world.register::<Children>();

        let parent = world.spawn();
        let entities = (0..3).map(|_| world.spawn()).collect::<Vec<_>>();

        AddChildren {
            parent,
            children: entities.clone(),
        }
        .execute(&mut world);

        RemoveAllChildren(parent).execute(&mut world);

        let children = world.get_component::<Children>(parent);

        assert!(children.is_none());
        for child in entities {
            let parent = world.get_component::<Parent>(child);
            assert!(parent.is_none());
        }
    }
}
