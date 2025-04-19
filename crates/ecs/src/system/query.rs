use crate::core::{Frame, ObjectStatus, blob::Ptr, sparse::SparseIndex};
use crate::system::Access;
use crate::world::{
    Component, ComponentId, Entity, World,
    archetype::{
        Archetype, ArchetypeQuery,
        table::{Column, RowIndex},
    },
    cell::WorldCell,
};

use super::arg::SystemArg;
use super::{SystemAccess, SystemInit};

pub trait BaseQuery {
    type Item<'w>;
    type State<'w>;

    /// Data used to construct the state of the query.
    /// This is used to create the query state when the query is first created.
    type Data: Send + Sync + Sized;

    fn init(system: &mut SystemInit, query: &mut ArchetypeQuery) -> Self::Data;

    fn state<'w>(
        data: &'w Self::Data,
        world: WorldCell<'w>,
        archetype: &'w Archetype,
        current_frame: Frame,
        system_frame: Frame,
    ) -> Self::State<'w>;

    fn get<'w>(state: &mut Self::State<'w>, entity: Entity, row: RowIndex) -> Self::Item<'w>;

    fn access(_: &Self::Data) -> Vec<SystemAccess> {
        vec![]
    }
}

impl BaseQuery for () {
    type Item<'w> = ();

    type State<'w> = ();

    type Data = ();

    fn init(_: &mut SystemInit, _: &mut ArchetypeQuery) -> Self::Data {
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

pub trait BaseFilter {
    type State<'w>;
    type Data: Send + Sync + Sized;

    fn init(system: &mut SystemInit, query: &mut ArchetypeQuery) -> Self::Data;

    fn state<'w>(
        data: &Self::Data,
        world: WorldCell<'w>,
        archetype: &'w Archetype,
        current_frame: Frame,
        system_frame: Frame,
    ) -> Self::State<'w>;

    fn filter<'w>(state: &Self::State<'w>, entity: Entity, row: RowIndex) -> bool;
}

impl BaseFilter for () {
    type State<'w> = ();
    type Data = ();

    fn init(_: &mut SystemInit, _: &mut ArchetypeQuery) -> Self::Data {
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

pub struct Not<F: BaseFilter>(std::marker::PhantomData<F>);
impl<C: Component> BaseFilter for Not<Added<C>> {
    type State<'w> = <Added<C> as BaseFilter>::State<'w>;

    type Data = <Added<C> as BaseFilter>::Data;

    fn init(system: &mut SystemInit, _: &mut ArchetypeQuery) -> Self::Data {
        system.world.register::<C>()
    }

    fn state<'w>(
        data: &Self::Data,
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

    fn init(system: &mut SystemInit, _: &mut ArchetypeQuery) -> Self::Data {
        system.world.register::<C>()
    }

    fn state<'w>(
        data: &Self::Data,
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

pub struct With<'w, C: Component>(std::marker::PhantomData<&'w C>);
impl<C: Component> BaseFilter for With<'_, C> {
    type State<'w> = bool;
    type Data = ComponentId;

    fn init(system: &mut SystemInit, query: &mut ArchetypeQuery) -> Self::Data {
        let id = system.world.register::<C>();

        query.any(id)
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

    fn init(system: &mut SystemInit, query: &mut ArchetypeQuery) -> Self::Data {
        let id = system.world.register::<C>();

        query.exclude(id)
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

pub struct Added<T: 'static>(std::marker::PhantomData<T>);
pub struct AddedComponent<'w, C: Component> {
    reader: Option<ReadQuery<'w, C>>,
    current_frame: Frame,
    system_frame: Frame,
}

impl<C: Component> BaseFilter for Added<C> {
    type State<'w> = AddedComponent<'w, C>;
    type Data = ComponentId;

    fn init(system: &mut SystemInit, _: &mut ArchetypeQuery) -> Self::Data {
        system.world.register::<C>()
    }

    fn state<'w>(
        data: &Self::Data,
        _: WorldCell<'w>,
        archetype: &'w Archetype,
        current_frame: Frame,
        system_frame: Frame,
    ) -> Self::State<'w> {
        let components = archetype.table().get_column(*data);
        AddedComponent {
            reader: components.map(|components| ReadQuery::from(components)),
            current_frame,
            system_frame,
        }
    }

    fn filter<'w>(state: &Self::State<'w>, _: Entity, row: RowIndex) -> bool {
        match state.reader.as_ref() {
            Some(reader) => {
                let added_frame = reader.components.frames()[row.to_usize()].added;
                added_frame.is_newer(state.current_frame, state.system_frame)
            }
            None => false,
        }
    }
}

pub struct Modified<T: 'static>(std::marker::PhantomData<T>);
pub struct ModifiedComponent<'w, C: Component> {
    reader: Option<ReadQuery<'w, C>>,
    current_frame: Frame,
    system_frame: Frame,
}

impl<C: Component> BaseFilter for Modified<C> {
    type State<'w> = ModifiedComponent<'w, C>;
    type Data = ComponentId;

    fn init(system: &mut SystemInit, query: &mut ArchetypeQuery) -> Self::Data {
        let id = system.world.register::<C>();

        query.any(id)
    }

    fn state<'w>(
        data: &Self::Data,
        _: WorldCell<'w>,
        archetype: &'w Archetype,
        current_frame: Frame,
        system_frame: Frame,
    ) -> Self::State<'w> {
        let components = archetype.table().get_column(*data);
        ModifiedComponent {
            reader: components.map(|components| ReadQuery::from(components)),
            current_frame,
            system_frame,
        }
    }

    fn filter<'w>(state: &Self::State<'w>, _: Entity, row: RowIndex) -> bool {
        match state.reader.as_ref() {
            Some(reader) => {
                let modified = reader.components.frames()[row.to_usize()].modified;
                modified.is_newer(state.current_frame, state.system_frame)
            }
            None => false,
        }
    }
}

pub struct Or<'w, F: BaseFilter>(std::marker::PhantomData<&'w F>);

pub struct ReadQuery<'a, C: Component> {
    components: &'a Column,
    _marker: std::marker::PhantomData<C>,
}

impl<'a, C: Component> From<&'a Column> for ReadQuery<'a, C> {
    fn from(components: &'a Column) -> Self {
        Self {
            components,
            _marker: std::marker::PhantomData,
        }
    }
}

impl<C: Component> BaseQuery for &C {
    type Item<'w> = &'w C;

    type State<'w> = ReadQuery<'w, C>;

    type Data = ComponentId;

    fn init(system: &mut SystemInit, query: &mut ArchetypeQuery) -> Self::Data {
        let id = system.world.register::<C>();

        query.include(id)
    }

    fn state<'w>(
        data: &Self::Data,
        _: WorldCell<'w>,
        archetype: &'w Archetype,
        _: Frame,
        _: Frame,
    ) -> Self::State<'w> {
        let components = archetype.table().get_column(*data).expect(&format!(
            "Component not found in archetype: {}",
            std::any::type_name::<C>()
        ));

        ReadQuery::from(components)
    }

    fn get<'w>(state: &mut Self::State<'w>, entity: Entity, row: RowIndex) -> Self::Item<'w> {
        state
            .components
            .get(row.to_usize())
            .expect(&format!("Component not found for entity: {:?}", entity))
    }

    fn access(data: &Self::Data) -> Vec<SystemAccess> {
        vec![SystemAccess::Component {
            id: *data,
            access: Access::Read,
        }]
    }
}

pub struct WriteQuery<'a, C: Component> {
    components: Ptr<'a, C>,
    frames: Ptr<'a, ObjectStatus>,
    current_frame: Frame,
    _marker: std::marker::PhantomData<C>,
}

impl<'a, C: Component> WriteQuery<'a, C> {
    pub fn new(
        components: Ptr<'a, C>,
        frames: Ptr<'a, ObjectStatus>,
        current_frame: Frame,
    ) -> Self {
        Self {
            components,
            frames,
            current_frame,
            _marker: std::marker::PhantomData,
        }
    }
}

impl<C: Component> BaseQuery for &mut C {
    type Item<'w> = &'w mut C;

    type State<'w> = WriteQuery<'w, C>;

    type Data = ComponentId;

    fn init(system: &mut SystemInit, query: &mut ArchetypeQuery) -> Self::Data {
        <&C as BaseQuery>::init(system, query)
    }

    fn state<'w>(
        data: &Self::Data,
        _: WorldCell<'w>,
        archetype: &'w Archetype,
        current_frame: Frame,
        _: Frame,
    ) -> Self::State<'w> {
        let (components, frames) = unsafe {
            archetype
                .table()
                .get_column(*data)
                .expect(&format!(
                    "Component not found in archetype: {}",
                    std::any::type_name::<C>()
                ))
                .get_ptr()
        };

        WriteQuery::new(components, frames, current_frame)
    }

    fn get<'w>(state: &mut Self::State<'w>, entity: Entity, row: RowIndex) -> Self::Item<'w> {
        let component = unsafe {
            state.frames.get_mut(row.0 as usize).unwrap().modified = state.current_frame;

            state
                .components
                .get_mut(row.to_usize())
                .expect(&format!("Component not found for entity: {:?}", entity))
        };

        component
    }

    fn access(data: &Self::Data) -> Vec<SystemAccess> {
        vec![SystemAccess::Component {
            id: *data,
            access: Access::Write,
        }]
    }
}

impl<C: Component> BaseQuery for Option<&C> {
    type Item<'w> = Option<&'w C>;

    type State<'w> = Option<ReadQuery<'w, C>>;

    type Data = ComponentId;

    fn init(system: &mut SystemInit, _: &mut ArchetypeQuery) -> Self::Data {
        let id = system.world.register::<C>();

        id
    }

    fn state<'w>(
        data: &Self::Data,
        _: WorldCell<'w>,
        archetype: &'w Archetype,
        _: Frame,
        _: Frame,
    ) -> Self::State<'w> {
        archetype
            .table()
            .get_column(*data)
            .map(|column| ReadQuery::from(column))
    }

    fn get<'w>(state: &mut Self::State<'w>, entity: Entity, row: RowIndex) -> Self::Item<'w> {
        match state {
            Some(state) => Some(<&C as BaseQuery>::get(state, entity, row)),
            None => None,
        }
    }

    fn access(data: &Self::Data) -> Vec<SystemAccess> {
        <&C as BaseQuery>::access(data)
    }
}

impl<C: Component> BaseQuery for Option<&mut C> {
    type Item<'w> = Option<&'w mut C>;

    type State<'w> = Option<WriteQuery<'w, C>>;

    type Data = ComponentId;

    fn init(system: &mut SystemInit, _: &mut ArchetypeQuery) -> Self::Data {
        let id = system.world.register::<C>();

        id
    }

    fn state<'w>(
        data: &Self::Data,
        _: WorldCell<'w>,
        archetype: &'w Archetype,
        current_frame: Frame,
        _: Frame,
    ) -> Self::State<'w> {
        archetype.table().get_column(*data).map(|column| {
            let (components, frames) = unsafe { column.get_ptr() };
            WriteQuery::new(components, frames, current_frame)
        })
    }

    fn get<'w>(state: &mut Self::State<'w>, entity: Entity, row: RowIndex) -> Self::Item<'w> {
        match state {
            Some(state) => Some(<&mut C as BaseQuery>::get(state, entity, row)),
            None => None,
        }
    }

    fn access(data: &Self::Data) -> Vec<SystemAccess> {
        <&mut C as BaseQuery>::access(data)
    }
}

impl BaseQuery for Entity {
    type Item<'w> = Entity;

    type State<'w> = ();

    type Data = ();

    fn init(_: &mut SystemInit, _: &mut ArchetypeQuery) -> Self::Data {
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

    fn get<'w>(_: &mut Self::State<'w>, entity: Entity, _: RowIndex) -> Self::Item<'w> {
        entity
    }
}

pub struct QueryState<Q: BaseQuery, F: BaseFilter = ()> {
    pub(crate) query: ArchetypeQuery,
    pub(crate) data: Q::Data,
    pub(crate) filter: F::Data,
}

impl<Q: BaseQuery, F: BaseFilter> QueryState<Q, F> {
    pub fn new(system: &mut SystemInit) -> Self {
        let mut query = ArchetypeQuery::default();
        let data = Q::init(system, &mut query);
        let filter = F::init(system, &mut query);

        QueryState {
            query,
            data,
            filter,
        }
    }
}

pub struct Query<'w, 's, Q: BaseQuery, F: BaseFilter = ()> {
    world: WorldCell<'w>,
    state: &'s QueryState<Q, F>,
    current_frame: Frame,
    system_frame: Frame,
}

impl<'w, 's, Q: BaseQuery, F: BaseFilter> Query<'w, 's, Q, F> {
    pub fn new(world: &'w World, state: &'s QueryState<Q, F>) -> Self {
        Self {
            world: unsafe { WorldCell::new(world) },
            current_frame: world.frame(),
            system_frame: world.frame().previous(),
            state,
        }
    }

    pub fn with_frame(world: &'w World, state: &'s QueryState<Q, F>, frame: Frame) -> Self {
        Self {
            world: unsafe { WorldCell::new(world) },
            current_frame: world.frame(),
            system_frame: frame,
            state,
        }
    }

    pub fn iter(&'w self) -> QueryIter<'w, 's, Q, F> {
        QueryIter::new(&self)
    }

    pub fn contains(&self, entity: Entity) -> bool {
        let world = unsafe { self.world.get() };
        let Some(archetype) = world.archetypes().entity_archetype(entity) else {
            return false;
        };

        let Some(row) = archetype.table().get_entity_row(entity) else {
            return false;
        };

        let filter = F::state(
            &self.state.filter,
            self.world,
            archetype,
            self.current_frame,
            self.system_frame,
        );

        F::filter(&filter, entity, row) && archetype.matches(&self.state.query)
    }
}

unsafe impl<Q: BaseQuery + 'static, F: BaseFilter + 'static> SystemArg for Query<'_, '_, Q, F> {
    type Item<'world, 'state> = Query<'world, 'state, Q, F>;

    type State = QueryState<Q, F>;

    fn init(system: &mut SystemInit) -> Self::State {
        QueryState::new(system)
    }

    unsafe fn get<'world, 'state>(
        state: &'state mut Self::State,
        world: WorldCell<'world>,
        system: &super::SystemMeta,
    ) -> Self::Item<'world, 'state> {
        unsafe { Query::with_frame(world.get(), state, system.frame) }
    }

    fn access(state: &Self::State) -> Vec<super::SystemAccess> {
        Q::access(&state.data)
    }
}

pub struct QueryIter<'w, 's, Q: BaseQuery, F: BaseFilter = ()> {
    query: &'w Query<'w, 's, Q, F>,
    archetypes: Vec<&'w Archetype>,
    state: Option<Q::State<'w>>,
    filter: Option<F::State<'w>>,
    entities: Option<indexmap::set::Iter<'w, Entity>>,
    archetype: usize,
}

impl<'w, 's, Q: BaseQuery, F: BaseFilter> QueryIter<'w, 's, Q, F> {
    pub fn new(query: &'w Query<'w, 's, Q, F>) -> Self {
        let world = unsafe { query.world.get() };
        let archetypes = world.archetypes().query(&query.state.query);

        let (state, filter_state, entities) = archetypes
            .get(0)
            .map(|archetype| {
                let state = Q::state(
                    &query.state.data,
                    query.world,
                    archetype,
                    query.current_frame,
                    query.system_frame,
                );
                let filter_state = F::state(
                    &query.state.filter,
                    query.world,
                    archetype,
                    query.current_frame,
                    query.system_frame,
                );

                let entities = archetype.table().entities();

                (Some(state), Some(filter_state), Some(entities))
            })
            .unwrap_or((None, None, None));

        Self {
            query,
            archetypes,
            state,
            filter: filter_state,
            entities,
            archetype: 0,
        }
    }

    pub fn contains(&self, entity: Entity) -> bool {
        self.query.contains(entity)
    }
}

impl<'w, 's, Q: BaseQuery, F: BaseFilter> Iterator for QueryIter<'w, 's, Q, F> {
    type Item = Q::Item<'w>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.archetype >= self.archetypes.len() {
            None
        } else if let Some(entity) = self
            .entities
            .as_mut()
            .and_then(|entities| entities.next())
            .copied()
        {
            let row = self.archetypes[self.archetype]
                .table()
                .get_entity_row(entity)
                .unwrap();

            let state = self.state.as_mut()?;
            let filter = match &mut self.filter {
                Some(state) => F::filter(state, entity, row),
                None => true,
            };

            filter.then_some(Q::get(state, entity, row))
        } else {
            self.archetype += 1;
            self.entities = self.archetypes.get(self.archetype).map(|archetype| {
                self.state = Some(Q::state(
                    &self.query.state.data,
                    self.query.world,
                    archetype,
                    self.query.current_frame,
                    self.query.system_frame,
                ));
                self.filter = Some(F::state(
                    &self.query.state.filter,
                    self.query.world,
                    archetype,
                    self.query.current_frame,
                    self.query.system_frame,
                ));
                archetype.table().entities()
            });

            self.next()
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

                fn init(system: &mut SystemInit, query: &mut ArchetypeQuery) -> Self::Data {
                    ($($name::init(system, query),)*)
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

                fn access(data: &Self::Data) -> Vec<SystemAccess> {
                    let ($($name,)*) = data;
                    let mut access = vec![];
                    $(
                        access.extend($name::access($name));
                    )*
                    access
                }
            }

            #[allow(non_snake_case)]
            impl<$($name: BaseFilter),+> BaseFilter for ($($name),+) {
                type State<'w> = ($($name::State<'w>), +);

                type Data = ($($name::Data), +);

                fn init(system: &mut SystemInit, query: &mut ArchetypeQuery) -> Self::Data {
                    ($($name::init(system, query),)*)
                }

                fn state<'w>(data: &Self::Data, world: WorldCell<'w>, archetype: &'w Archetype, current_frame: Frame, system_frame: Frame) -> Self::State<'w> {
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
            impl<$($name: BaseFilter),+> BaseFilter for Or<'_, ($($name),+)> {
                type State<'w> = ($($name::State<'w>), +);

                type Data = ($($name::Data), +);

                fn init(system: &mut SystemInit, query: &mut ArchetypeQuery) -> Self::Data {
                    ($($name::init(system, query),)*)
                }

                fn state<'w>(data: &Self::Data, world: WorldCell<'w>, archetype: &'w Archetype, current_frame: Frame, system_frame: Frame) -> Self::State<'w> {
                    let ($($name,)*) = data;
                    ($($name::state($name, world, archetype, current_frame, system_frame),)*)
                }

                fn filter<'w>(state: &Self::State<'w>, entity: Entity, row: RowIndex) -> bool {
                    let ($($name,)*) = state;
                    let mut filter = true;

                    ($(
                        filter = filter || $name::filter($name, entity, row),
                    )*);

                    filter
                }
            }
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

#[cfg(test)]
mod tests {

    use crate::system::SystemMeta;

    use super::*;

    #[derive(Debug, PartialEq, Eq)]
    struct Age(u32);
    impl Component for Age {}

    #[derive(Debug, PartialEq, Eq)]
    struct Name(&'static str);
    impl Component for Name {}

    #[test]
    fn test_query() {
        let mut world = World::new();
        world.register::<Age>();
        world.register::<Name>();

        let entity = world.spawn();
        world.add_component(entity, Age(0));
        world.add_component(entity, Name("Bob"));

        let mut system = SystemInit::new(&mut world, None);
        let state = QueryState::<(&Age, &Name)>::new(&mut system);
        let query = Query::new(&world, &state);
        let iter = query.iter();

        assert!(iter.contains(entity));
    }

    #[test]
    fn test_with_query() {
        let mut world = World::new();
        world.register::<Age>();
        world.register::<Name>();

        let entity = world.spawn();
        world.add_component(entity, Age(0));
        world.add_component(entity, Name("Bob"));

        let mut system = SystemInit::new(&mut world, None);
        let state = QueryState::<&Age, With<Name>>::new(&mut system);
        let query = Query::new(&world, &state);

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

        let mut system = SystemInit::new(&mut world, None);
        let state = QueryState::<&Age, Without<Name>>::new(&mut system);
        let query = Query::new(&world, &state);

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

        let mut system = SystemInit::new(&mut world, None);
        let mut state = Query::<&Age, Added<Name>>::init(&mut system);
        let system = SystemMeta::default();
        let query = unsafe { Query::get(&mut state, world.cell(), &system) };

        assert!(query.contains(entity))
    }

    #[test]
    fn test_not_query() {
        let mut world = World::new();
        world.register::<Age>();
        world.register::<Name>();

        let entity_a = world.spawn();
        world.add_component(entity_a, Age(0));
        world.add_component(entity_a, Name("Bob"));

        let entity_b = world.spawn();
        world.add_component(entity_b, Age(1));

        let mut system = SystemInit::new(&mut world, None);
        let state = QueryState::<&Age, Not<Added<Name>>>::new(&mut system);
        let query = Query::new(&world, &state);

        assert!(!query.contains(entity_a));
        assert!(query.contains(entity_b));
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

        let mut system = SystemInit::new(&mut world, None);
        let state = QueryState::<Entity, Or<(With<Age>, With<Name>)>>::new(&mut system);
        let query = Query::new(&world, &state);

        assert!(query.contains(entity_a));
        assert!(query.contains(entity_b));
        assert!(!query.contains(entity_c));
    }
}
