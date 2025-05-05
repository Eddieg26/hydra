use crate::{BaseFilter, BaseQuery, Children, Entity, Parent, Query, QueryIterState};

const EMPTY: &'static [Entity] = &[];

pub trait HierarchyExt<Q: BaseQuery, F: BaseFilter> {
    fn parent(&self, entity: Entity) -> Option<Q::Item<'_>>;
    fn children(&self, entity: Entity) -> ChildIter<Q, F>;
    fn ancestors(&self, entity: Entity) -> AncestorIter<Q, F>;
    fn descendents(&self, entity: Entity) -> DescendentIter<Q, F>;
}

impl<'w, 's, Q: BaseQuery, F: BaseFilter> HierarchyExt<Q, F> for Query<'w, 's, Q, F> {
    fn parent(&self, entity: Entity) -> Option<<Q as BaseQuery>::Item<'_>> {
        let world = unsafe { self.world.get() };
        let archetype = world.archetypes.entity_archetype(entity)?;

        let mut state = QueryIterState::new(self, archetype);
        let row = archetype.table().get_entity_row(entity).unwrap();

        if F::filter(&state.filter, entity, row) {
            Some(Q::get(&mut state.data, entity, row))
        } else {
            None
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
