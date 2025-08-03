use super::{Added, Modified, Not, Or, ReadOnly, Removed, SystemArg};
use crate::{
    Archetype, ArchetypeAccess, ArchetypeId, ArchetypeQuery, Component, ComponentId, Entity,
    EntityEvents, Event, Frame, SparseIndex, World,
    core::{ObjectStatus, Ptr},
    world::{Column, Events, RowIndex, WorldCell},
};

pub trait BaseQuery {
    type Item<'w>;
    type State<'w>;

    /// Data used to construct the state of the query.
    /// This is used to create the query state when the query is first created.
    type Data: Send + Sync + Sized;

    fn init(world: &mut World, access: &mut ArchetypeAccess) -> Self::Data;

    fn state<'w>(
        data: &'w Self::Data,
        world: WorldCell<'w>,
        archetype: &'w Archetype,
        current_frame: Frame,
        system_frame: Frame,
    ) -> Self::State<'w>;

    fn get<'w>(state: &mut Self::State<'w>, entity: Entity, row: RowIndex) -> Self::Item<'w>;
}

pub unsafe trait ReadQuery: BaseQuery {}

pub trait BaseFilter {
    type State<'w>;
    type Data: Send + Sync + Sized;

    fn init(world: &mut World, access: &mut ArchetypeAccess) -> Self::Data;

    fn state<'w>(
        data: &'w Self::Data,
        world: WorldCell<'w>,
        archetype: &'w Archetype,
        current_frame: Frame,
        system_frame: Frame,
    ) -> Self::State<'w>;

    fn filter<'w>(state: &Self::State<'w>, entity: Entity, row: RowIndex) -> bool;
}

impl BaseQuery for () {
    type Item<'w> = ();

    type State<'w> = ();

    type Data = ();

    fn init(_: &mut World, _: &mut ArchetypeAccess) -> Self::Data {
        ()
    }

    fn state<'w>(
        _: &'w Self::Data,
        _: WorldCell<'w>,
        _: &'w Archetype,
        _: Frame,
        _: Frame,
    ) -> Self::State<'w> {
        ()
    }

    fn get<'w>(_: &mut Self::State<'w>, _: Entity, _: RowIndex) -> Self::Item<'w> {
        ()
    }
}

unsafe impl ReadQuery for () {}

impl BaseFilter for () {
    type State<'w> = ();

    type Data = ();

    fn init(_: &mut World, _: &mut ArchetypeAccess) -> Self::Data {
        ()
    }

    fn state<'w>(
        _: &Self::Data,
        _: WorldCell<'w>,
        _: &'w Archetype,
        _: Frame,
        _: Frame,
    ) -> Self::State<'w> {
        ()
    }

    fn filter<'w>(_: &Self::State<'w>, _: Entity, _: RowIndex) -> bool {
        true
    }
}

impl BaseQuery for Entity {
    type Item<'w> = Entity;

    type State<'w> = ();

    type Data = ();

    fn init(_: &mut World, _: &mut ArchetypeAccess) -> Self::Data {
        ()
    }

    fn state<'w>(
        _: &'w Self::Data,
        _: WorldCell<'w>,
        _: &'w Archetype,
        _: Frame,
        _: Frame,
    ) -> Self::State<'w> {
        ()
    }

    fn get<'w>(_: &mut Self::State<'w>, entity: Entity, _: RowIndex) -> Self::Item<'w> {
        entity
    }
}

unsafe impl ReadQuery for Entity {}

pub struct ReadPtr<'w, C: Component> {
    components: &'w Column,
    current_frame: Frame,
    system_frame: Frame,
    _marker: std::marker::PhantomData<C>,
}

impl<'w, C: Component> ReadPtr<'w, C> {
    pub fn new(components: &'w Column, current_frame: Frame, system_frame: Frame) -> Self {
        Self {
            components,
            current_frame,
            system_frame,
            _marker: Default::default(),
        }
    }
}

impl<C: Component> BaseQuery for &C {
    type Item<'w> = &'w C;

    type State<'w> = ReadPtr<'w, C>;

    type Data = ComponentId;

    fn init(world: &mut World, access: &mut ArchetypeAccess) -> Self::Data {
        access.read(world.register::<C>())
    }

    fn state<'w>(
        data: &'w Self::Data,
        _: WorldCell<'w>,
        archetype: &'w Archetype,
        current_frame: Frame,
        system_frame: Frame,
    ) -> Self::State<'w> {
        match archetype.table().get_column(*data) {
            Some(column) => ReadPtr::new(column, current_frame, system_frame),
            None => {
                let component = std::any::type_name::<C>();
                panic!("Component does not exist in this archetype: {}", component);
            }
        }
    }

    fn get<'w>(state: &mut Self::State<'w>, _: Entity, row: RowIndex) -> Self::Item<'w> {
        state.components.get::<C>(row.to_usize()).unwrap()
    }
}

unsafe impl<C: Component> ReadQuery for &C {}

pub struct WritePtr<'w, C: Component> {
    components: Ptr<'w, C>,
    frames: Ptr<'w, ObjectStatus>,
    current_frame: Frame,
}

impl<'w, C: Component> WritePtr<'w, C> {
    pub fn new(
        components: Ptr<'w, C>,
        frames: Ptr<'w, ObjectStatus>,
        current_frame: Frame,
    ) -> Self {
        Self {
            components,
            frames,
            current_frame,
        }
    }
}

impl<C: Component> BaseQuery for &mut C {
    type Item<'w> = &'w mut C;

    type State<'w> = WritePtr<'w, C>;

    type Data = ComponentId;

    fn init(world: &mut World, access: &mut ArchetypeAccess) -> Self::Data {
        access.write(world.register::<C>())
    }

    fn state<'w>(
        data: &'w Self::Data,
        _: WorldCell<'w>,
        archetype: &'w Archetype,
        current_frame: Frame,
        _: Frame,
    ) -> Self::State<'w> {
        match archetype.table().get_column(*data) {
            Some(column) => {
                let (components, frames) = unsafe { column.get_ptr::<C>() };
                WritePtr::new(components, frames, current_frame)
            }
            None => {
                let component = std::any::type_name::<C>();
                panic!("Component does not exist in this archetype: {}", component);
            }
        }
    }

    fn get<'w>(state: &mut Self::State<'w>, _: Entity, row: RowIndex) -> Self::Item<'w> {
        unsafe {
            state.frames.get_mut(row.to_usize()).added = state.current_frame;
            state.components.get_mut(row.to_usize())
        }
    }
}

impl<C: Component> BaseQuery for Option<&C> {
    type Item<'w> = Option<<&'w C as BaseQuery>::Item<'w>>;

    type State<'w> = Option<<&'w C as BaseQuery>::State<'w>>;

    type Data = <&'static C as BaseQuery>::Data;

    fn init(world: &mut World, access: &mut ArchetypeAccess) -> Self::Data {
        access.read_optional(world.register::<C>())
    }

    fn state<'w>(
        data: &'w Self::Data,
        world: WorldCell<'w>,
        archetype: &'w Archetype,
        current_frame: Frame,
        system_frame: Frame,
    ) -> Self::State<'w> {
        if archetype.has_component_id(*data) {
            Some(<&C as BaseQuery>::state(
                data,
                world,
                archetype,
                current_frame,
                system_frame,
            ))
        } else {
            None
        }
    }

    fn get<'w>(state: &mut Self::State<'w>, entity: Entity, row: RowIndex) -> Self::Item<'w> {
        state
            .as_mut()
            .map(|state| <&C as BaseQuery>::get(state, entity, row))
    }
}

unsafe impl<C: Component> ReadQuery for Option<&C> {}

impl<C: Component> BaseQuery for Option<&mut C> {
    type Item<'w> = Option<<&'w mut C as BaseQuery>::Item<'w>>;

    type State<'w> = Option<<&'w mut C as BaseQuery>::State<'w>>;

    type Data = <&'static mut C as BaseQuery>::Data;

    fn init(world: &mut World, access: &mut ArchetypeAccess) -> Self::Data {
        access.write_optional(world.register::<C>())
    }

    fn state<'w>(
        data: &'w Self::Data,
        world: WorldCell<'w>,
        archetype: &'w Archetype,
        current_frame: Frame,
        system_frame: Frame,
    ) -> Self::State<'w> {
        if archetype.has_component_id(*data) {
            Some(<&mut C as BaseQuery>::state(
                data,
                world,
                archetype,
                current_frame,
                system_frame,
            ))
        } else {
            None
        }
    }

    fn get<'w>(state: &mut Self::State<'w>, entity: Entity, row: RowIndex) -> Self::Item<'w> {
        state
            .as_mut()
            .map(|state| <&mut C as BaseQuery>::get(state, entity, row))
    }
}

pub struct With<C: Component>(std::marker::PhantomData<C>);
impl<C: Component> BaseFilter for With<C> {
    type State<'w> = bool;

    type Data = ComponentId;

    fn init(world: &mut World, access: &mut ArchetypeAccess) -> Self::Data {
        let id = world.register::<C>();
        access.include(id);
        id
    }

    fn state<'w>(
        data: &Self::Data,
        _: WorldCell<'w>,
        archetype: &'w Archetype,
        _: Frame,
        _: Frame,
    ) -> Self::State<'w> {
        archetype.has_component_id(*data)
    }

    fn filter<'w>(state: &Self::State<'w>, _: Entity, _: RowIndex) -> bool {
        *state
    }
}

pub struct Without<C: Component>(std::marker::PhantomData<C>);
impl<C: Component> BaseFilter for Without<C> {
    type State<'w> = bool;

    type Data = ComponentId;

    fn init(world: &mut World, access: &mut ArchetypeAccess) -> Self::Data {
        access.exclude(world.register::<C>())
    }

    fn state<'w>(
        data: &Self::Data,
        _: WorldCell<'w>,
        archetype: &'w Archetype,
        _: Frame,
        _: Frame,
    ) -> Self::State<'w> {
        !archetype.has_component_id(*data)
    }

    fn filter<'w>(state: &Self::State<'w>, _: Entity, _: RowIndex) -> bool {
        *state
    }
}

impl<C: Component> BaseFilter for Added<C> {
    type State<'w> = ReadPtr<'w, C>;

    type Data = ComponentId;

    fn init(world: &mut World, access: &mut ArchetypeAccess) -> Self::Data {
        let id = world.register::<C>();
        access.include(id)
    }

    fn state<'w>(
        data: &'w Self::Data,
        world: WorldCell<'w>,
        archetype: &'w Archetype,
        current_frame: Frame,
        system_frame: Frame,
    ) -> Self::State<'w> {
        <&C as BaseQuery>::state(data, world, archetype, current_frame, system_frame)
    }

    fn filter<'w>(state: &Self::State<'w>, _: Entity, row: RowIndex) -> bool {
        let status = state.components.frames().get(row.to_usize()).unwrap();
        status
            .added
            .is_newer(state.current_frame, state.system_frame)
    }
}

impl<C: Component> BaseFilter for Modified<C> {
    type State<'w> = ReadPtr<'w, C>;

    type Data = ComponentId;

    fn init(world: &mut World, access: &mut ArchetypeAccess) -> Self::Data {
        let id = world.register::<C>();
        access.include(id)
    }

    fn state<'w>(
        data: &'w Self::Data,
        world: WorldCell<'w>,
        archetype: &'w Archetype,
        current_frame: Frame,
        system_frame: Frame,
    ) -> Self::State<'w> {
        <&C as BaseQuery>::state(data, world, archetype, current_frame, system_frame)
    }

    fn filter<'w>(state: &Self::State<'w>, _: Entity, row: RowIndex) -> bool {
        let status = state.components.frames().get(row.to_usize()).unwrap();
        status
            .modified
            .is_newer(state.current_frame, state.system_frame)
    }
}

impl<C: Component> BaseFilter for Not<Added<C>> {
    type State<'w> = <Added<C> as BaseFilter>::State<'w>;

    type Data = <Added<C> as BaseFilter>::Data;

    fn init(world: &mut World, access: &mut ArchetypeAccess) -> Self::Data {
        <Added<C> as BaseFilter>::init(world, access)
    }

    fn state<'w>(
        data: &'w Self::Data,
        world: WorldCell<'w>,
        archetype: &'w Archetype,
        current_frame: Frame,
        system_frame: Frame,
    ) -> Self::State<'w> {
        <Added<C> as BaseFilter>::state(data, world, archetype, current_frame, system_frame)
    }

    fn filter<'w>(state: &Self::State<'w>, entity: Entity, row: RowIndex) -> bool {
        !<Added<C> as BaseFilter>::filter(state, entity, row)
    }
}

impl<C: Component> BaseFilter for Not<Modified<C>> {
    type State<'w> = <Modified<C> as BaseFilter>::State<'w>;

    type Data = <Modified<C> as BaseFilter>::Data;

    fn init(world: &mut World, access: &mut ArchetypeAccess) -> Self::Data {
        <Modified<C> as BaseFilter>::init(world, access)
    }

    fn state<'w>(
        data: &'w Self::Data,
        world: WorldCell<'w>,
        archetype: &'w Archetype,
        current_frame: Frame,
        system_frame: Frame,
    ) -> Self::State<'w> {
        <Modified<C> as BaseFilter>::state(data, world, archetype, current_frame, system_frame)
    }

    fn filter<'w>(state: &Self::State<'w>, entity: Entity, row: RowIndex) -> bool {
        !<Modified<C> as BaseFilter>::filter(state, entity, row)
    }
}

impl<E: Event> BaseFilter for E {
    type State<'w> = &'w Events<E>;

    type Data = ();

    fn init(world: &mut World, _: &mut ArchetypeAccess) -> Self::Data {
        world.register_event::<E>();
    }

    fn state<'w>(
        _: &Self::Data,
        world: WorldCell<'w>,
        _: &'w Archetype,
        _: Frame,
        _: Frame,
    ) -> Self::State<'w> {
        unsafe { world.get() }.resource::<Events<E>>()
    }

    fn filter<'w>(state: &Self::State<'w>, entity: Entity, _: RowIndex) -> bool {
        state
            .read
            .entities
            .get(&entity)
            .map(|events| !events.is_empty())
            .unwrap_or(false)
    }
}

impl<E: Event> BaseQuery for Events<E> {
    type Item<'w> = EntityEvents<'w, E>;

    type State<'w> = &'w Events<E>;

    type Data = ();

    fn init(world: &mut World, _: &mut ArchetypeAccess) -> Self::Data {
        world.register_event::<E>();
    }

    fn state<'w>(
        _: &Self::Data,
        world: WorldCell<'w>,
        _: &'w Archetype,
        _: Frame,
        _: Frame,
    ) -> Self::State<'w> {
        unsafe { world.get() }.resource::<Events<E>>()
    }

    fn get<'w>(state: &mut Self::State<'w>, entity: Entity, _: RowIndex) -> Self::Item<'w> {
        EntityEvents::new(&state, state.entity(entity))
    }
}

impl<C: Component> Event for Removed<C> {}

impl<C: Component> BaseQuery for Removed<C> {
    type Item<'w> = EntityEvents<'w, Self>;

    type State<'w> = &'w Events<Self>;

    type Data = ();

    fn init(world: &mut World, _: &mut ArchetypeAccess) -> Self::Data {
        world.register_event::<Self>();
    }

    fn state<'w>(
        _: &Self::Data,
        world: WorldCell<'w>,
        _: &'w Archetype,
        _: Frame,
        _: Frame,
    ) -> Self::State<'w> {
        unsafe { world.get() }.resource::<Events<Self>>()
    }

    fn get<'w>(state: &mut Self::State<'w>, entity: Entity, _: RowIndex) -> Self::Item<'w> {
        EntityEvents::new(&state, state.entity(entity))
    }
}

pub struct QueryState<Q: BaseQuery, F: BaseFilter = ()> {
    access: ArchetypeQuery,
    archetypes: Vec<ArchetypeId>,
    last_archetype: usize,
    data: Q::Data,
    filter: F::Data,
}

impl<Q: BaseQuery, F: BaseFilter> QueryState<Q, F> {
    pub fn new(world: &mut World, access: &mut ArchetypeAccess) -> Self {
        let data = Q::init(world, access);
        let filter = F::init(world, access);

        Self {
            access: ArchetypeQuery::from(access.clone()),
            archetypes: Vec::new(),
            last_archetype: 0,
            data,
            filter,
        }
    }

    pub fn access(&self) -> &ArchetypeQuery {
        &self.access
    }

    pub fn archetypes(&self) -> &[ArchetypeId] {
        &self.archetypes
    }

    pub fn data(&self) -> &Q::Data {
        &self.data
    }

    pub fn filter(&self) -> &F::Data {
        &self.filter
    }

    pub fn update(&mut self, world: &mut World) {
        let archetypes = world.archetypes.archetypes();
        for archetype in archetypes.iter().skip(self.last_archetype) {
            if archetype.matches(&self.access) {
                self.archetypes.push(archetype.id());
            }
        }

        self.last_archetype = archetypes.len();
    }
}

pub struct Query<'w, 's, Q: BaseQuery, F: BaseFilter = ()> {
    pub(crate) world: WorldCell<'w>,
    pub(crate) state: &'s QueryState<Q, F>,
    pub(crate) current_frame: Frame,
    pub(crate) system_frame: Frame,
}

impl<'w, 's, Q: BaseQuery, F: BaseFilter> Query<'w, 's, Q, F> {
    pub fn new(world: WorldCell<'w>, state: &'s mut QueryState<Q, F>) -> Self {
        Self::with_frame(world, state, Frame::ZERO)
    }

    pub fn with_frame(
        mut world: WorldCell<'w>,
        state: &'s mut QueryState<Q, F>,
        frame: Frame,
    ) -> Self {
        state.update(unsafe { world.get_mut() });

        Self {
            world,
            state,
            current_frame: unsafe { world.get().frame },
            system_frame: frame,
        }
    }

    pub fn iter(&'w self) -> QueryIter<'w, 's, Q, F> {
        QueryIter::new(self)
    }

    pub fn contains(&self, entity: Entity) -> bool {
        let world = unsafe { self.world.get() };
        let Some(archetype) = world.archetypes().entity_archetype(entity) else {
            return false;
        };

        let Some(row) = archetype.table().get_entity_row(entity) else {
            return false;
        };

        if !self.state.archetypes.contains(&archetype.id()) {
            return false;
        }

        let filter = F::state(
            &self.state.filter,
            self.world,
            archetype,
            self.current_frame,
            self.system_frame,
        );

        F::filter(&filter, entity, row)
    }

    pub fn get_item<'a>(&'a self, entity: Entity) -> Option<Q::Item<'a>> {
        let archetype = match unsafe { self.world.get() }
            .archetypes
            .entity_archetype(entity)
        {
            Some(archetype) => archetype,
            None => return None,
        };

        let mut state = QueryIterState::new(self, archetype);

        let row = archetype.table().get_entity_row(entity).unwrap();

        match F::filter(&state.filter, entity, row) {
            true => Some(Q::get(&mut state.data, entity, row)),
            false => None,
        }
    }
}

unsafe impl<Q: BaseQuery + 'static, F: BaseFilter + 'static> SystemArg for Query<'_, '_, Q, F> {
    type Item<'world, 'state> = Query<'world, 'state, Q, F>;

    type State = QueryState<Q, F>;

    fn init(world: &mut World, access: &mut crate::WorldAccess) -> Self::State {
        let mut archetype = ArchetypeAccess::new();
        let state = QueryState::new(world, &mut archetype);
        access.add_archetype(archetype);

        state
    }

    unsafe fn get<'world, 'state>(
        state: &'state mut Self::State,
        world: WorldCell<'world>,
        system: &super::SystemMeta,
    ) -> Self::Item<'world, 'state> {
        Query::with_frame(world, state, system.frame)
    }
}

unsafe impl<Q: ReadQuery + 'static, F: BaseFilter + 'static> ReadOnly for Query<'_, '_, Q, F> {}

impl<'w, 's, Q: BaseQuery, F: BaseFilter> IntoIterator for &'w Query<'w, 's, Q, F> {
    type Item = Q::Item<'w>;

    type IntoIter = QueryIter<'w, 's, Q, F>;

    fn into_iter(self) -> Self::IntoIter {
        QueryIter::new(self)
    }
}

pub struct QueryIterState<'a, Q: BaseQuery, F: BaseFilter = ()> {
    pub(crate) data: Q::State<'a>,
    pub(crate) filter: F::State<'a>,
}

impl<'a, Q: BaseQuery, F: BaseFilter> QueryIterState<'a, Q, F> {
    pub fn new(query: &'a Query<'a, 'a, Q, F>, archetype: &'a Archetype) -> Self {
        let data = Q::state(
            &query.state.data,
            query.world,
            archetype,
            query.current_frame,
            query.system_frame,
        );

        let filter = F::state(
            &query.state.filter,
            query.world,
            archetype,
            query.current_frame,
            query.system_frame,
        );

        Self { data, filter }
    }

    pub fn data(&'a self) -> &'a Q::State<'a> {
        &self.data
    }

    pub fn filter(&'a self) -> &'a F::State<'a> {
        &self.filter
    }
}

pub struct QueryIter<'w, 's, Q: BaseQuery, F: BaseFilter = ()> {
    query: &'w Query<'w, 's, Q, F>,
    archetypes: Vec<&'w Archetype>,
    state: Option<QueryIterState<'w, Q, F>>,
    entities: Option<indexmap::set::Iter<'w, Entity>>,
    archetype: usize,
}

impl<'w, 's, Q: BaseQuery, F: BaseFilter> QueryIter<'w, 's, Q, F> {
    pub fn new(query: &'w Query<'w, 's, Q, F>) -> Self {
        let world = unsafe { query.world.get() };
        let archetypes = world.archetypes().archetypes();
        let archetypes = query
            .state
            .archetypes
            .iter()
            .map(|a| &archetypes[a.to_usize()])
            .collect::<Vec<_>>();

        let (state, entities) = match archetypes.first() {
            Some(archetype) => {
                let state = QueryIterState::new(query, archetype);
                let entities = archetype.table().entities();
                (Some(state), Some(entities))
            }
            None => (None, None),
        };

        Self {
            query,
            archetypes,
            state,
            entities,
            archetype: 0,
        }
    }
}

impl<'w, 's, Q: BaseQuery, F: BaseFilter> Iterator for QueryIter<'w, 's, Q, F> {
    type Item = Q::Item<'w>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.archetype >= self.archetypes.len() {
            None
        } else if let Some(entity) = self.entities.as_mut().and_then(|e| e.next()).copied() {
            let Some(index) = self.archetypes[self.archetype.to_usize()].get_entity(entity) else {
                return self.next();
            };

            let Some(state) = self.state.as_mut() else {
                return self.next();
            };

            match F::filter(&state.filter, entity, index.row) {
                true => Some(Q::get(&mut state.data, entity, index.row)),
                false => None,
            }
        } else {
            self.archetype += 1;

            let (state, entities) = match self.archetypes.get(self.archetype) {
                Some(archetype) => {
                    let state = QueryIterState::new(self.query, archetype);
                    let entities = archetype.table().entities();
                    (Some(state), Some(entities))
                }
                None => (None, None),
            };

            self.entities = entities;
            self.state = state;

            return self.next();
        }
    }
}

#[macro_export]
macro_rules! impl_base_query_for_tuples {
    ($(($($name:ident),*)),*)  => {
        $(
            #[allow(non_snake_case)]
            impl<$($name: BaseQuery),+> BaseQuery for ($($name),+) {
                type Item<'w> = ($($name::Item<'w>), +);

                type State<'w> = ($($name::State<'w>), +);

                type Data = ($($name::Data), +);

                fn init(world: &mut World, access: &mut ArchetypeAccess) -> Self::Data {
                    ($($name::init(world, access),)*)
                }

                fn state<'w>(data: &'w Self::Data, world: WorldCell<'w>, archetype: &'w Archetype, current_frame: Frame, system_frame: Frame) -> Self::State<'w> {
                    let ($($name,)*) = data;
                    ($($name::state($name, world, archetype, current_frame, system_frame),)*)
                }

                fn get<'w>(state: &mut Self::State<'w>, entity: Entity, row: RowIndex) -> Self::Item<'w> {
                    let ($($name,)*) = state;

                    ($(
                        $name::get($name, entity, row),
                    )*)
                }
            }

            #[allow(non_snake_case)]
            impl<$($name: BaseFilter),+> BaseFilter for ($($name),+) {
                type State<'w> = ($($name::State<'w>), +);

                type Data = ($($name::Data), +);

                fn init(world: &mut World, access: &mut ArchetypeAccess) -> Self::Data {
                    ($($name::init(world, access),)*)
                }

                fn state<'w>(data: &'w Self::Data, world: WorldCell<'w>, archetype: &'w Archetype, current_frame: Frame, system_frame: Frame) -> Self::State<'w> {
                    let ($($name,)*) = data;
                    ($($name::state($name, world, archetype, current_frame, system_frame),)*)
                }

                fn filter<'w>(state: &Self::State<'w>, entity: Entity, row: RowIndex) -> bool {
                    let ($($name,)*) = state;
                    let mut filter = true;

                    ($(
                        filter = filter && $name::filter($name, entity, row),
                    )*);

                    filter
                }
            }

            #[allow(non_snake_case)]
            impl<$($name: BaseFilter),+> BaseFilter for Or<($($name),+)> {
                type State<'w> = ($($name::State<'w>), +);

                type Data = ($($name::Data), +);

                fn init(world: &mut World, access: &mut ArchetypeAccess) -> Self::Data {
                    ($($name::init(world, access),)*)
                }

                fn state<'w>(data: &'w Self::Data, world: WorldCell<'w>, archetype: &'w Archetype, current_frame: Frame, system_frame: Frame) -> Self::State<'w> {
                    let ($($name,)*) = data;
                    ($($name::state($name, world, archetype, current_frame, system_frame),)*)
                }

                fn filter<'w>(state: &Self::State<'w>, entity: Entity, row: RowIndex) -> bool {
                    let ($($name,)*) = state;
                    let mut filter = false;

                    ($(
                        filter = filter || $name::filter($name, entity, row),
                    )*);

                    filter
                }
            }

            unsafe impl<$($name: ReadQuery),+> ReadQuery for ($($name),+) {}

            unsafe impl<$($name: ReadOnly),+> ReadOnly for ($($name),+) {}
        )+
    };
}

impl_base_query_for_tuples!((A, B));
impl_base_query_for_tuples!((A, B, C));
impl_base_query_for_tuples!((A, B, C, D));
impl_base_query_for_tuples!((A, B, C, D, E));
impl_base_query_for_tuples!((A, B, C, D, E, F));
impl_base_query_for_tuples!((A, B, C, D, E, F, G));
impl_base_query_for_tuples!((A, B, C, D, E, F, G, H));
impl_base_query_for_tuples!((A, B, C, D, E, F, G, H, I));
impl_base_query_for_tuples!((A, B, C, D, E, F, G, H, I, J));
impl_base_query_for_tuples!((A, B, C, D, E, F, G, H, I, J, K));
impl_base_query_for_tuples!((A, B, C, D, E, F, G, H, I, J, K, L));
impl_base_query_for_tuples!((A, B, C, D, E, F, G, H, I, J, K, L, M));
impl_base_query_for_tuples!((A, B, C, D, E, F, G, H, I, J, K, L, M, N));
impl_base_query_for_tuples!((A, B, C, D, E, F, G, H, I, J, K, L, M, N, O));
impl_base_query_for_tuples!((A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P));
impl_base_query_for_tuples!((A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q));

#[allow(unused_imports, dead_code)]
mod tests {
    use super::*;
    use crate::{
        ComponentKit, Components, WorldAccess,
        system::SystemMeta,
        world::{ComponentRemover, ComponentWriter},
    };

    #[derive(Debug, Default, PartialEq, Eq)]
    struct Age(u32);
    impl Component for Age {}

    #[derive(Debug, Default, PartialEq, Eq)]
    struct Name(&'static str);
    impl Component for Name {}

    #[derive(Debug, Default, PartialEq, Eq, ComponentKit)]
    struct Person {
        age: Age,
        name: Name,
    }

    #[test]
    fn test_query() {
        let mut world = World::new();
        world.register::<Age>();
        world.register::<Name>();

        let entity = world.spawn();
        world.add_component(entity, Age(0));
        world.add_component(entity, Name("Bob"));

        let mut state = QueryState::<(&Age, &Name)>::new(&mut world, &mut ArchetypeAccess::new());
        let query = Query::new(unsafe { world.cell() }, &mut state);

        assert!(query.contains(entity));
    }

    #[test]
    fn test_with_query() {
        let mut world = World::new();
        world.register::<Age>();
        world.register::<Name>();

        let entity = world.spawn();
        world.add_component(entity, Age(0));
        world.add_component(entity, Name("Bob"));

        let mut state =
            QueryState::<&Age, With<Name>>::new(&mut world, &mut ArchetypeAccess::new());
        let query = Query::new(unsafe { world.cell() }, &mut state);

        assert!(query.contains(entity));
    }

    #[test]
    fn test_without_query() {
        let mut world = World::new();
        world.register::<Age>();
        world.register::<Name>();

        let entity = world.spawn();
        world.add_component(entity, Age(0));
        world.add_component(entity, Name("Bob"));

        let mut state =
            QueryState::<&Age, Without<Name>>::new(&mut world, &mut ArchetypeAccess::new());
        let query = Query::new(unsafe { world.cell() }, &mut state);

        assert!(!query.contains(entity));
    }

    #[test]
    fn test_added_query() {
        let mut world = World::new();
        world.register::<Age>();
        world.register::<Name>();

        let entity = world.spawn();
        world.add_component(entity, Age(0));
        world.add_component(entity, Name("Bob"));

        let mut state =
            QueryState::<&Age, Added<Name>>::new(&mut world, &mut ArchetypeAccess::new());
        let query = Query::new(unsafe { world.cell() }, &mut state);

        assert!(query.contains(entity));
    }

    #[test]
    fn test_remove_component() {
        let mut world = World::new();
        world.register::<Age>();

        let entity = world.spawn();
        world.add_component(entity, Age(0));
        world.remove_component::<Age>(entity);

        world.update();

        let mut state = QueryState::<(Entity, Removed<Age>), Removed<Age>>::new(
            &mut world,
            &mut ArchetypeAccess::new(),
        );

        let query = Query::new(unsafe { world.cell() }, &mut state);
        let (entity, mut events) = query.iter().next().unwrap();
        let event = events.next().unwrap();

        assert!(query.contains(entity));
        assert_eq!(**event, Age(0));
    }

    #[test]
    fn test_remove_components() {
        let mut world = World::new();
        world.register::<Age>();
        world.register::<Name>();

        let entity = world.spawn();
        world.add_components(entity, Person::default());
        world.remove_components::<Person>(entity);

        world.update();

        let mut state = QueryState::<
            (Entity, Removed<Age>, Removed<Name>),
            (Removed<Age>, Removed<Name>),
        >::new(&mut world, &mut ArchetypeAccess::new());

        let query = Query::new(unsafe { world.cell() }, &mut state);
        let (entity, mut age, mut name) = query.iter().next().unwrap();
        let age = age.next().unwrap();
        let name = name.next().unwrap();

        assert!(query.contains(entity));
        assert_eq!(**age, Age(0));
        assert_eq!(**name, Name(""));
    }

    #[test]
    fn test_not_query() {
        let mut world = World::new();
        world.register::<Age>();
        world.register::<Name>();

        let entity_a = world.spawn();
        world.add_component(entity_a, Age(0));
        world.add_component(entity_a, Name("Bob"));

        let mut state =
            QueryState::<&Age, Not<Added<Name>>>::new(&mut world, &mut ArchetypeAccess::new());
        let query = Query::new(unsafe { world.cell() }, &mut state);

        assert!(!query.contains(entity_a));
    }

    #[test]
    fn test_or_query() {
        let mut world = World::new();
        world.register::<Age>();
        world.register::<Name>();

        let entity_a = world.spawn();
        let entity_b = world.spawn();
        let entity_c = world.spawn();
        world.add_component(entity_a, Age(32));
        world.add_component(entity_b, Name("Bob"));

        let mut state = QueryState::<Entity, Or<(With<Age>, With<Name>)>>::new(
            &mut world,
            &mut ArchetypeAccess::new(),
        );
        let query = Query::new(unsafe { world.cell() }, &mut state);

        assert!(query.contains(entity_a));
        assert!(query.contains(entity_b));
        assert!(!query.contains(entity_c));
    }

    #[test]
    fn test_disjoint_query() {
        let mut world = World::new();

        let mut access = ArchetypeAccess::new();
        QueryState::<&Age, With<Name>>::new(&mut world, &mut access);

        let mut other = ArchetypeAccess::new();
        QueryState::<&mut Age, Without<Name>>::new(&mut world, &mut other);

        assert!(access.is_disjoint(&other))
    }

    #[test]
    fn test_intersected_query() {
        let mut world = World::new();

        let mut access = ArchetypeAccess::new();
        QueryState::<&Age, With<Name>>::new(&mut world, &mut access);

        let mut other = ArchetypeAccess::new();
        QueryState::<&mut Age, With<Name>>::new(&mut world, &mut other);

        assert!(!access.is_disjoint(&other))
    }
}
