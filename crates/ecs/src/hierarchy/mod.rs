use std::ops::Index;

use crate::{BaseFilter, BaseQuery, Component, Entity, Query};

const EMPTY: &'static [Entity] = &[];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Parent(Entity);
impl Parent {
    pub(crate) fn set(&mut self, entity: Entity) -> Entity {
        let old = self.0;
        self.0 = entity;
        old
    }

    pub fn get(&self) -> Entity {
        self.0
    }
}

impl From<Entity> for Parent {
    fn from(value: Entity) -> Self {
        Parent(value)
    }
}

impl std::ops::Deref for Parent {
    type Target = Entity;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Component for Parent {}

#[derive(Debug, Clone, Hash)]
pub struct Children(Vec<Entity>);

impl Children {
    pub fn as_slice(&self) -> &[Entity] {
        self.0.as_slice()
    }

    pub fn get(&self, index: usize) -> Option<Entity> {
        self.0.get(index).copied()
    }

    pub fn sort(&mut self) {
        self.0.sort();
    }

    pub fn sort_by(&mut self, compare: impl FnMut(&Entity, &Entity) -> std::cmp::Ordering) {
        self.0.sort_by(compare);
    }

    pub fn contains(&self, child: Entity) -> bool {
        self.0.contains(&child)
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub(crate) fn push(&mut self, child: Entity) {
        self.0.push(child);
    }

    pub(crate) fn insert(&mut self, index: usize, children: Vec<Entity>) {
        self.0.splice(index..index, children);
    }

    pub(crate) fn remove(&mut self, child: Entity) -> bool {
        self.0
            .iter()
            .position(|c| *c == child)
            .map(|i| self.0.remove(i))
            .is_some()
    }

    pub(crate) fn retain(&mut self, f: impl FnMut(&Entity) -> bool) {
        self.0.retain(f);
    }
}

impl Index<usize> for Children {
    type Output = Entity;

    fn index(&self, index: usize) -> &Self::Output {
        &self.0[index]
    }
}

impl<I: IntoIterator<Item = Entity>> From<I> for Children {
    fn from(value: I) -> Self {
        let mut children = value.into_iter().collect::<Vec<_>>();
        children.dedup();

        Self(children)
    }
}

impl From<Entity> for Children {
    fn from(value: Entity) -> Self {
        Self(vec![value])
    }
}

impl Component for Children {}

pub trait HierarchyExt<Q: BaseQuery, F: BaseFilter> {
    fn parent(&self, entity: Entity) -> Option<Q::Item<'_>>;
    fn children(&self, entity: Entity) -> ChildIter<Q, F>;
    fn ancestors(&self, entity: Entity) -> AncestorIter<Q, F>;
    fn descendents(&self, entity: Entity) -> DescendentIter<Q, F>;
}

impl<'w, 's, Q: BaseQuery, F: BaseFilter> HierarchyExt<Q, F> for Query<'w, 's, Q, F> {
    fn parent(&self, entity: Entity) -> Option<<Q as BaseQuery>::Item<'_>> {
        let world = unsafe { self.world.get() };
        let parent = world.get_component::<Parent>(entity)?.get();

        match self.get_item(parent) {
            Ok(item) => item,
            Err(_) => None,
        }
    }

    fn children(&self, entity: Entity) -> ChildIter<Q, F> {
        ChildIter::new(entity, self)
    }

    fn ancestors(&self, entity: Entity) -> AncestorIter<Q, F> {
        AncestorIter::new(entity, self)
    }

    fn descendents(&self, entity: Entity) -> DescendentIter<Q, F> {
        DescendentIter::new(entity, self)
    }
}

pub struct EntityIter<'a, Q: BaseQuery, F: BaseFilter, P: Fn(&Entity, &Entity) -> bool> {
    entity: Entity,
    query: &'a Query<'a, 'a, Q, F>,
    entities: std::slice::Iter<'a, Entity>,
    predicate: P,
}

impl<'a, Q: BaseQuery, F: BaseFilter, P: Fn(&Entity, &Entity) -> bool> EntityIter<'a, Q, F, P> {
    pub fn new(
        entity: Entity,
        query: &'a Query<'a, 'a, Q, F>,
        entities: std::slice::Iter<'a, Entity>,
        predicate: P,
    ) -> Self {
        Self {
            entity,
            query,
            entities,
            predicate,
        }
    }
}

impl<'a, Q: BaseQuery, F: BaseFilter, P: Fn(&Entity, &Entity) -> bool> Iterator
    for EntityIter<'a, Q, F, P>
{
    type Item = Q::Item<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let entity = self.entities.next().copied()?;
        if !(self.predicate)(&self.entity, &entity) {
            return self.next();
        }

        match self.query.get_item(entity) {
            Ok(result) => result,
            Err(_) => return self.next(),
        }
    }
}

pub struct ChildIter<'a, Q: BaseQuery, F: BaseFilter> {
    inner: EntityIter<'a, Q, F, fn(&Entity, &Entity) -> bool>,
}

impl<'a, Q: BaseQuery, F: BaseFilter> ChildIter<'a, Q, F> {
    pub fn new(entity: Entity, query: &'a Query<'a, 'a, Q, F>) -> Self {
        let world = unsafe { query.world.get() };

        match world.get_component::<Children>(entity) {
            Some(children) => {
                let entities = children.as_slice().iter();
                Self {
                    inner: EntityIter::new(entity, query, entities, |_, _| true),
                }
            }
            None => Self {
                inner: EntityIter::new(entity, query, [].iter(), |_, _| true),
            },
        }
    }
}

impl<'a, Q: BaseQuery, F: BaseFilter> Iterator for ChildIter<'a, Q, F> {
    type Item = Q::Item<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}

pub struct AncestorIter<'a, Q: BaseQuery, F: BaseFilter> {
    query: &'a Query<'a, 'a, Q, F>,
    current: Option<Entity>,
}

impl<'a, Q: BaseQuery, F: BaseFilter> AncestorIter<'a, Q, F> {
    pub fn new(entity: Entity, query: &'a Query<'a, 'a, Q, F>) -> Self {
        Self {
            query,
            current: Some(entity),
        }
    }
}

impl<'a, Q: BaseQuery, F: BaseFilter> Iterator for AncestorIter<'a, Q, F> {
    type Item = Q::Item<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let entity = self.current.take()?;

        let item = self.query.parent(entity);
        self.current = unsafe { self.query.world.get() }
            .get_component::<Parent>(entity)
            .map(|p| p.get());

        item
    }
}

pub struct SiblingIter<'a, Q: BaseQuery, F: BaseFilter> {
    inner: EntityIter<'a, Q, F, fn(&Entity, &Entity) -> bool>,
}

impl<'a, Q: BaseQuery, F: BaseFilter> SiblingIter<'a, Q, F> {
    pub fn new(entity: Entity, query: &'a Query<'a, 'a, Q, F>) -> Self {
        let world = unsafe { query.world.get() };

        let entities = world
            .get_component::<Parent>(entity)
            .and_then(|p| world.get_component::<Children>(Parent::get(p)))
            .map(|children| children.as_slice().iter())
            .unwrap_or(EMPTY.iter());

        Self {
            inner: EntityIter::new(entity, query, entities, |entity, child| entity != child),
        }
    }
}

impl<'a, Q: BaseQuery, F: BaseFilter> Iterator for SiblingIter<'a, Q, F> {
    type Item = Q::Item<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}

pub struct DescendentIter<'a, Q: BaseQuery, F: BaseFilter> {
    query: &'a Query<'a, 'a, Q, F>,
    entities: Vec<std::slice::Iter<'a, Entity>>,
    index: usize,
}

impl<'a, Q: BaseQuery, F: BaseFilter> DescendentIter<'a, Q, F> {
    pub fn new(entity: Entity, query: &'a Query<'a, 'a, Q, F>) -> Self {
        let world = unsafe { query.world.get() };

        let entities = world
            .get_component::<Parent>(entity)
            .and_then(|p| world.get_component::<Children>(Parent::get(p)))
            .map(|children| vec![children.as_slice().iter()])
            .unwrap_or_default();

        Self {
            query,
            entities,
            index: 0,
        }
    }
}

impl<'a, Q: BaseQuery, F: BaseFilter> Iterator for DescendentIter<'a, Q, F> {
    type Item = Q::Item<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.entities.len() {
            return None;
        }

        let entity = match self.entities[self.index].next().copied() {
            Some(entity) => entity,
            None => {
                self.index += 1;
                return self.next();
            }
        };

        if let Some(children) = unsafe { self.query.world.get() }.get_component::<Children>(entity)
        {
            self.entities.push(children.as_slice().iter());
        }

        match self.query.get_item(entity) {
            Ok(result) => result,
            Err(_) => return self.next(),
        }
    }
}
