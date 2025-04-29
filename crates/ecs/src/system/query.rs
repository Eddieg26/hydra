// use super::arg::SystemArg;
// use super::{Access, SystemInit, SystemName};
// use crate::core::{Frame, ObjectStatus, blob::Ptr, sparse::SparseIndex};
// use crate::world::{
//     Component, ComponentId, Entity, World,
//     archetype::{
//         Archetype, ArchetypeAccess,
//         table::{Column, RowIndex},
//     },
//     cell::WorldCell,
// };
// use crate::{AccessBitset, EntityEvents, Event, Events};

// pub struct QueryBuilder<'a> {
//     system: Option<&'a SystemName>,
//     world: &'a mut World,
//     access: &'a mut AccessBitset,
//     query: &'a mut ArchetypeAccess,
// }

// impl<'a> QueryBuilder<'a> {
//     pub fn world(&mut self) -> &mut World {
//         self.world
//     }

//     pub fn include<C: Component>(&mut self, access: Access) -> ComponentId {
//         let id = match access {
//             Access::Read => self.read::<C>(),
//             Access::Write => self.write::<C>(),
//         };

//         self.query.required(id)
//     }

//     pub fn exclude<C: Component>(&mut self) -> ComponentId {
//         let id = self.world.register::<C>();

//         self.access.set(id.to_usize(), false);
//         self.access.set(id.to_usize() + 1, false);

//         self.query.exclude(id)
//     }

//     pub fn any<C: Component>(&mut self) -> ComponentId {
//         let id = self.world.register::<C>();

//         self.query.include(id)
//     }

//     pub fn read<C: Component>(&mut self) -> ComponentId {
//         let id = self.world.register::<C>();

//         if !self.access.read(id.to_usize()) {
//             let meta = self.world.components().get_by_id(id).unwrap();

//             panic!("{:?}: Invalid read access: {}", self.system, meta.name());
//         }

//         id
//     }

//     pub fn write<C: Component>(&mut self) -> ComponentId {
//         let id = self.world.register::<C>();

//         if !self.access.write(id.to_usize()) {
//             let meta = self.world.components().get_by_id(id).unwrap();

//             panic!("{:?}: Invalid write access: {}", self.system, meta.name());
//         }

//         id
//     }
// }

// pub trait BaseQuery {
//     type Item<'w>;
//     type State<'w>;

//     /// Data used to construct the state of the query.
//     /// This is used to create the query state when the query is first created.
//     type Data: Send + Sync + Sized;

//     fn init(builder: &mut QueryBuilder) -> Self::Data;

//     fn state<'w>(
//         data: &'w Self::Data,
//         world: WorldCell<'w>,
//         archetype: &'w Archetype,
//         current_frame: Frame,
//         system_frame: Frame,
//     ) -> Self::State<'w>;

//     fn get<'w>(state: &mut Self::State<'w>, entity: Entity, row: RowIndex) -> Self::Item<'w>;
// }

// pub unsafe trait ReadQuery: BaseQuery {}

// pub trait ComponentQuery {
//     type Component: Component;
//     type Query: BaseQuery;
//     type Filter: BaseFilter;

//     type Item<'w>: IntoIterator<Item = <Self::Query as BaseQuery>::Item<'w>>;

//     fn get<'w>(
//         state: &mut SubQueryState<'w, Self::Query, Self::Filter>,
//         entity: Entity,
//         row: RowIndex,
//     ) -> Self::Item<'w>;
// }

// pub struct SubQuery<C: ComponentQuery>(std::marker::PhantomData<C>);

// pub struct SubQueryState<'w, Q: BaseQuery, F: BaseFilter = ()> {
//     pub query: &'w ArchetypeAccess,
//     pub world: WorldCell<'w>,
//     pub column: Option<&'w Column>,
//     pub state: Q::State<'w>,
//     pub filter: F::State<'w>,
// }

// impl<C: ComponentQuery> BaseQuery for SubQuery<C> {
//     type Item<'w> = C::Item<'w>;

//     type State<'w> = SubQueryState<'w, C::Query, C::Filter>;

//     type Data = (QueryState<C::Query, C::Filter>, ComponentId);

//     fn init(builder: &mut QueryBuilder) -> Self::Data {
//         let mut sub_query = ArchetypeAccess::default();
//         let mut access = AccessBitset::new();

//         let mut sub_query_builder = QueryBuilder {
//             system: builder.system.clone(),
//             world: builder.world,
//             access: &mut access,
//             query: &mut sub_query,
//         };

//         let data = C::Query::init(&mut sub_query_builder);
//         let filter = C::Filter::init(&mut sub_query_builder);
//         let id = sub_query_builder.world.register::<C::Component>();

//         for (index, (read, write)) in sub_query_builder.access.iter().enumerate() {
//             let valid = if write {
//                 builder.access.write(id.to_usize())
//             } else if read {
//                 builder.access.read(id.to_usize())
//             } else {
//                 true
//             };

//             if !valid {
//                 let component = builder
//                     .world
//                     .components()
//                     .get_by_id(ComponentId(index as u32))
//                     .unwrap();
//                 let system = builder.system;

//                 panic!(
//                     "Invalid subquery access for component: {} at system: {:?}",
//                     component.name(),
//                     system
//                 );
//             }
//         }

//         let state = QueryState {
//             query: sub_query,
//             data,
//             filter,
//         };

//         (state, id)
//     }

//     fn state<'w>(
//         data: &'w Self::Data,
//         world: WorldCell<'w>,
//         archetype: &'w Archetype,
//         current_frame: Frame,
//         system_frame: Frame,
//     ) -> Self::State<'w> {
//         let column = match archetype.matches(&data.0.query) {
//             true => None,
//             false => archetype.table().get_column(data.1),
//         };
//         let state = C::Query::state(&data.0.data, world, archetype, current_frame, system_frame);
//         let filter = C::Filter::state(
//             &data.0.filter,
//             world,
//             archetype,
//             current_frame,
//             system_frame,
//         );

//         SubQueryState {
//             query: &data.0.query,
//             world,
//             column,
//             state,
//             filter,
//         }
//     }

//     fn get<'w>(state: &mut Self::State<'w>, entity: Entity, row: RowIndex) -> Self::Item<'w> {
//         C::get(state, entity, row)
//     }
// }

// impl BaseQuery for () {
//     type Item<'w> = ();

//     type State<'w> = ();

//     type Data = ();

//     fn init(_: &mut QueryBuilder) -> Self::Data {
//         ()
//     }

//     fn state<'w>(
//         _: &'w Self::Data,
//         _: WorldCell<'w>,
//         _: &'w Archetype,
//         _: Frame,
//         _: Frame,
//     ) -> Self::State<'w> {
//         ()
//     }

//     fn get<'w>(_: &mut Self::State<'w>, _: Entity, _: RowIndex) -> Self::Item<'w> {
//         ()
//     }
// }

// unsafe impl ReadQuery for () {}

// pub trait BaseFilter {
//     type State<'w>;
//     type Data: Send + Sync + Sized;

//     fn init(builder: &mut QueryBuilder) -> Self::Data;

//     fn state<'w>(
//         data: &Self::Data,
//         world: WorldCell<'w>,
//         archetype: &'w Archetype,
//         current_frame: Frame,
//         system_frame: Frame,
//     ) -> Self::State<'w>;

//     fn filter<'w>(state: &Self::State<'w>, entity: Entity, row: RowIndex) -> bool;
// }

// impl BaseFilter for () {
//     type State<'w> = ();
//     type Data = ();

//     fn init(_: &mut QueryBuilder) -> Self::Data {
//         ()
//     }

//     fn state<'w>(
//         _: &Self::Data,
//         _: WorldCell<'w>,
//         _: &'w Archetype,
//         _: Frame,
//         _: Frame,
//     ) -> Self::State<'w> {
//         ()
//     }

//     fn filter<'w>(_: &Self::State<'w>, _: Entity, _: RowIndex) -> bool {
//         true
//     }
// }

// pub struct Not<F: BaseFilter>(std::marker::PhantomData<F>);
// impl<C: Component> BaseFilter for Not<Added<C>> {
//     type State<'w> = <Added<C> as BaseFilter>::State<'w>;

//     type Data = <Added<C> as BaseFilter>::Data;

//     fn init(builder: &mut QueryBuilder) -> Self::Data {
//         builder.world().register::<C>()
//     }

//     fn state<'w>(
//         data: &Self::Data,
//         world: WorldCell<'w>,
//         archetype: &'w Archetype,
//         current_frame: Frame,
//         system_frame: Frame,
//     ) -> Self::State<'w> {
//         <Added<C> as BaseFilter>::state(data, world, archetype, current_frame, system_frame)
//     }

//     fn filter<'w>(state: &Self::State<'w>, entity: Entity, row: RowIndex) -> bool {
//         !<Added<C> as BaseFilter>::filter(state, entity, row)
//     }
// }

// impl<C: Component> BaseFilter for Not<Modified<C>> {
//     type State<'w> = <Modified<C> as BaseFilter>::State<'w>;

//     type Data = <Modified<C> as BaseFilter>::Data;

//     fn init(builder: &mut QueryBuilder) -> Self::Data {
//         builder.world().register::<C>()
//     }

//     fn state<'w>(
//         data: &Self::Data,
//         world: WorldCell<'w>,
//         archetype: &'w Archetype,
//         current_frame: Frame,
//         system_frame: Frame,
//     ) -> Self::State<'w> {
//         <Modified<C> as BaseFilter>::state(data, world, archetype, current_frame, system_frame)
//     }

//     fn filter<'w>(state: &Self::State<'w>, entity: Entity, row: RowIndex) -> bool {
//         !<Modified<C> as BaseFilter>::filter(state, entity, row)
//     }
// }

// pub struct With<'w, C: Component>(std::marker::PhantomData<&'w C>);
// impl<C: Component> BaseFilter for With<'_, C> {
//     type State<'w> = bool;
//     type Data = ComponentId;

//     fn init(builder: &mut QueryBuilder) -> Self::Data {
//         builder.include::<C>(Access::Read)
//     }

//     fn state<'w>(
//         data: &Self::Data,
//         _: WorldCell<'w>,
//         archetype: &'w Archetype,
//         _: Frame,
//         _: Frame,
//     ) -> Self::State<'w> {
//         archetype.has_component_id(*data)
//     }

//     fn filter<'w>(state: &Self::State<'w>, _: Entity, _: RowIndex) -> bool {
//         *state
//     }
// }

// pub struct Without<C: Component>(std::marker::PhantomData<C>);
// impl<C: Component> BaseFilter for Without<C> {
//     type State<'w> = bool;
//     type Data = ComponentId;

//     fn init(builder: &mut QueryBuilder) -> Self::Data {
//         builder.exclude::<C>()
//     }

//     fn state<'w>(
//         data: &Self::Data,
//         _: WorldCell<'w>,
//         archetype: &'w Archetype,
//         _: Frame,
//         _: Frame,
//     ) -> Self::State<'w> {
//         !archetype.has_component_id(*data)
//     }

//     fn filter<'w>(state: &Self::State<'w>, _: Entity, _: RowIndex) -> bool {
//         *state
//     }
// }

// pub struct ReadFramePtr<'w, C: Component> {
//     reader: Option<ReadPtr<'w, C>>,
//     current_frame: Frame,
//     system_frame: Frame,
// }

// pub struct Added<T>(std::marker::PhantomData<T>);

// impl<C: Component> BaseFilter for Added<C> {
//     type State<'w> = ReadFramePtr<'w, C>;
//     type Data = ComponentId;

//     fn init(builder: &mut QueryBuilder) -> Self::Data {
//         builder.any::<C>()
//     }

//     fn state<'w>(
//         data: &Self::Data,
//         _: WorldCell<'w>,
//         archetype: &'w Archetype,
//         current_frame: Frame,
//         system_frame: Frame,
//     ) -> Self::State<'w> {
//         let components = archetype.table().get_column(*data);
//         ReadFramePtr {
//             reader: components.map(|components| ReadPtr::from(components)),
//             current_frame,
//             system_frame,
//         }
//     }

//     fn filter<'w>(state: &Self::State<'w>, _: Entity, row: RowIndex) -> bool {
//         match state.reader.as_ref() {
//             Some(reader) => {
//                 let added_frame = reader.components.frames()[row.to_usize()].added;
//                 added_frame.is_newer(state.current_frame, state.system_frame)
//             }
//             None => false,
//         }
//     }
// }

// pub struct Modified<T>(std::marker::PhantomData<T>);

// impl<C: Component> BaseFilter for Modified<C> {
//     type State<'w> = ReadFramePtr<'w, C>;
//     type Data = ComponentId;

//     fn init(builder: &mut QueryBuilder) -> Self::Data {
//         builder.any::<C>()
//     }

//     fn state<'w>(
//         data: &Self::Data,
//         _: WorldCell<'w>,
//         archetype: &'w Archetype,
//         current_frame: Frame,
//         system_frame: Frame,
//     ) -> Self::State<'w> {
//         let components = archetype.table().get_column(*data);
//         ReadFramePtr {
//             reader: components.map(|components| ReadPtr::from(components)),
//             current_frame,
//             system_frame,
//         }
//     }

//     fn filter<'w>(state: &Self::State<'w>, _: Entity, row: RowIndex) -> bool {
//         match state.reader.as_ref() {
//             Some(reader) => {
//                 let modified = reader.components.frames()[row.to_usize()].modified;
//                 modified.is_newer(state.current_frame, state.system_frame)
//             }
//             None => false,
//         }
//     }
// }

// pub struct Or<'w, F: BaseFilter>(std::marker::PhantomData<&'w F>);

// pub struct ReadPtr<'a, C: Component> {
//     components: &'a Column,
//     _marker: std::marker::PhantomData<C>,
// }

// impl<'a, C: Component> From<&'a Column> for ReadPtr<'a, C> {
//     fn from(components: &'a Column) -> Self {
//         Self {
//             components,
//             _marker: std::marker::PhantomData,
//         }
//     }
// }

// impl<C: Component> BaseQuery for &C {
//     type Item<'w> = &'w C;

//     type State<'w> = ReadPtr<'w, C>;

//     type Data = ComponentId;

//     fn init(builder: &mut QueryBuilder) -> Self::Data {
//         builder.include::<C>(Access::Read)
//     }

//     fn state<'w>(
//         data: &Self::Data,
//         _: WorldCell<'w>,
//         archetype: &'w Archetype,
//         _: Frame,
//         _: Frame,
//     ) -> Self::State<'w> {
//         let components = archetype.table().get_column(*data).expect(&format!(
//             "Component not found in archetype: {}",
//             std::any::type_name::<C>()
//         ));

//         ReadPtr::from(components)
//     }

//     fn get<'w>(state: &mut Self::State<'w>, entity: Entity, row: RowIndex) -> Self::Item<'w> {
//         state
//             .components
//             .get(row.to_usize())
//             .expect(&format!("Component not found for entity: {:?}", entity))
//     }
// }

// unsafe impl<C: Component> ReadQuery for &C {}

// pub struct WritePtr<'a, C: Component> {
//     components: Ptr<'a, C>,
//     frames: Ptr<'a, ObjectStatus>,
//     current_frame: Frame,
//     _marker: std::marker::PhantomData<C>,
// }

// impl<'a, C: Component> WritePtr<'a, C> {
//     pub fn new(
//         components: Ptr<'a, C>,
//         frames: Ptr<'a, ObjectStatus>,
//         current_frame: Frame,
//     ) -> Self {
//         Self {
//             components,
//             frames,
//             current_frame,
//             _marker: std::marker::PhantomData,
//         }
//     }
// }

// impl<C: Component> BaseQuery for &mut C {
//     type Item<'w> = &'w mut C;

//     type State<'w> = WritePtr<'w, C>;

//     type Data = ComponentId;

//     fn init(builder: &mut QueryBuilder) -> Self::Data {
//         builder.include::<C>(Access::Write)
//     }

//     fn state<'w>(
//         data: &Self::Data,
//         _: WorldCell<'w>,
//         archetype: &'w Archetype,
//         current_frame: Frame,
//         _: Frame,
//     ) -> Self::State<'w> {
//         let (components, frames) = unsafe {
//             archetype
//                 .table()
//                 .get_column(*data)
//                 .expect(&format!(
//                     "Component not found in archetype: {}",
//                     std::any::type_name::<C>()
//                 ))
//                 .get_ptr()
//         };

//         WritePtr::new(components, frames, current_frame)
//     }

//     fn get<'w>(state: &mut Self::State<'w>, entity: Entity, row: RowIndex) -> Self::Item<'w> {
//         let component = unsafe {
//             state.frames.get_mut(row.0 as usize).unwrap().modified = state.current_frame;

//             state
//                 .components
//                 .get_mut(row.to_usize())
//                 .expect(&format!("Component not found for entity: {:?}", entity))
//         };

//         component
//     }
// }

// impl<C: Component> BaseQuery for Option<&C> {
//     type Item<'w> = Option<&'w C>;

//     type State<'w> = Option<ReadPtr<'w, C>>;

//     type Data = ComponentId;

//     fn init(builder: &mut QueryBuilder) -> Self::Data {
//         builder.read::<C>()
//     }

//     fn state<'w>(
//         data: &Self::Data,
//         _: WorldCell<'w>,
//         archetype: &'w Archetype,
//         _: Frame,
//         _: Frame,
//     ) -> Self::State<'w> {
//         archetype
//             .table()
//             .get_column(*data)
//             .map(|column| ReadPtr::from(column))
//     }

//     fn get<'w>(state: &mut Self::State<'w>, entity: Entity, row: RowIndex) -> Self::Item<'w> {
//         match state {
//             Some(state) => Some(<&C as BaseQuery>::get(state, entity, row)),
//             None => None,
//         }
//     }
// }

// unsafe impl<C: Component> ReadQuery for Option<&C> {}

// impl<C: Component> BaseQuery for Option<&mut C> {
//     type Item<'w> = Option<&'w mut C>;

//     type State<'w> = Option<WritePtr<'w, C>>;

//     type Data = ComponentId;

//     fn init(builder: &mut QueryBuilder) -> Self::Data {
//         builder.write::<C>()
//     }

//     fn state<'w>(
//         data: &Self::Data,
//         _: WorldCell<'w>,
//         archetype: &'w Archetype,
//         current_frame: Frame,
//         _: Frame,
//     ) -> Self::State<'w> {
//         archetype.table().get_column(*data).map(|column| {
//             let (components, frames) = unsafe { column.get_ptr() };
//             WritePtr::new(components, frames, current_frame)
//         })
//     }

//     fn get<'w>(state: &mut Self::State<'w>, entity: Entity, row: RowIndex) -> Self::Item<'w> {
//         match state {
//             Some(state) => Some(<&mut C as BaseQuery>::get(state, entity, row)),
//             None => None,
//         }
//     }
// }

// impl BaseQuery for Entity {
//     type Item<'w> = Entity;

//     type State<'w> = ();

//     type Data = ();

//     fn init(_: &mut QueryBuilder) -> Self::Data {
//         ()
//     }

//     fn state<'w>(
//         _: &Self::Data,
//         _: WorldCell<'w>,
//         _: &'w Archetype,
//         _: Frame,
//         _: Frame,
//     ) -> Self::State<'w> {
//         ()
//     }

//     fn get<'w>(_: &mut Self::State<'w>, entity: Entity, _: RowIndex) -> Self::Item<'w> {
//         entity
//     }
// }

// unsafe impl ReadQuery for Entity {}

// impl<E: Event> BaseQuery for Events<E> {
//     type Item<'w> = EntityEvents<'w, E>;

//     type State<'w> = &'w Events<E>;

//     type Data = ();

//     fn init(builder: &mut QueryBuilder) -> Self::Data {
//         builder.world.register_event::<E>();
//     }

//     fn state<'w>(
//         _: &Self::Data,
//         world: WorldCell<'w>,
//         _: &'w Archetype,
//         _: Frame,
//         _: Frame,
//     ) -> Self::State<'w> {
//         unsafe { world.get() }.resource::<Events<E>>()
//     }

//     fn get<'w>(state: &mut Self::State<'w>, entity: Entity, _: RowIndex) -> Self::Item<'w> {
//         EntityEvents::new(&state, state.entity(entity))
//     }
// }

// unsafe impl<E: Event> ReadQuery for Events<E> {}

// impl<E: Event> BaseFilter for E {
//     type State<'w> = &'w Events<E>;

//     type Data = ();

//     fn init(builder: &mut QueryBuilder) -> Self::Data {
//         builder.world.register_event::<E>();
//     }

//     fn state<'w>(
//         _: &Self::Data,
//         world: WorldCell<'w>,
//         _: &'w Archetype,
//         _: Frame,
//         _: Frame,
//     ) -> Self::State<'w> {
//         unsafe { world.get() }.resource::<Events<E>>()
//     }

//     fn filter<'w>(state: &Self::State<'w>, entity: Entity, _: RowIndex) -> bool {
//         state
//             .read
//             .entities
//             .get(&entity)
//             .map(|events| !events.is_empty())
//             .unwrap_or(false)
//     }
// }

// pub struct QueryState<Q: BaseQuery, F: BaseFilter = ()> {
//     pub(crate) query: ArchetypeAccess,
//     pub(crate) data: Q::Data,
//     pub(crate) filter: F::Data,
// }

// impl<Q: BaseQuery, F: BaseFilter> QueryState<Q, F> {
//     pub fn new(system: &mut SystemInit) -> Self {
//         let mut query = ArchetypeAccess::default();
//         let world = unsafe { system.world.cell().get_mut() };

//         let mut builder = QueryBuilder {
//             system: system.name.as_ref(),
//             world,
//             access: &mut system.components,
//             query: &mut query,
//         };

//         let data = Q::init(&mut builder);
//         let filter = F::init(&mut builder);

//         QueryState {
//             query,
//             data,
//             filter,
//         }
//     }
// }

// pub struct Query<'w, 's, Q: BaseQuery, F: BaseFilter = ()> {
//     world: WorldCell<'w>,
//     state: &'s QueryState<Q, F>,
//     current_frame: Frame,
//     system_frame: Frame,
// }

// impl<'w, 's, Q: BaseQuery, F: BaseFilter> Query<'w, 's, Q, F> {
//     pub fn new(world: &'w World, state: &'s QueryState<Q, F>) -> Self {
//         Self {
//             world: unsafe { WorldCell::new(world) },
//             current_frame: world.frame(),
//             system_frame: world.frame().previous(),
//             state,
//         }
//     }

//     pub fn with_frame(world: &'w World, state: &'s QueryState<Q, F>, frame: Frame) -> Self {
//         Self {
//             world: unsafe { WorldCell::new(world) },
//             current_frame: world.frame(),
//             system_frame: frame,
//             state,
//         }
//     }

//     pub fn iter(&'w self) -> QueryIter<'w, 's, Q, F> {
//         QueryIter::new(&self)
//     }

//     pub fn contains(&self, entity: Entity) -> bool {
//         let world = unsafe { self.world.get() };
//         let Some(archetype) = world.archetypes().entity_archetype(entity) else {
//             return false;
//         };

//         let Some(row) = archetype.table().get_entity_row(entity) else {
//             return false;
//         };

//         let filter = F::state(
//             &self.state.filter,
//             self.world,
//             archetype,
//             self.current_frame,
//             self.system_frame,
//         );

//         F::filter(&filter, entity, row) && archetype.matches(&self.state.query)
//     }
// }

// unsafe impl<Q: BaseQuery + 'static, F: BaseFilter + 'static> SystemArg for Query<'_, '_, Q, F> {
//     type Item<'world, 'state> = Query<'world, 'state, Q, F>;

//     type State = QueryState<Q, F>;

//     fn init(system: &mut SystemInit) -> Self::State {
//         QueryState::new(system)
//     }

//     unsafe fn get<'world, 'state>(
//         state: &'state mut Self::State,
//         world: WorldCell<'world>,
//         system: &super::SystemMeta,
//     ) -> Self::Item<'world, 'state> {
//         unsafe { Query::with_frame(world.get(), state, system.frame) }
//     }
// }

// pub struct QueryIter<'w, 's, Q: BaseQuery, F: BaseFilter = ()> {
//     query: &'w Query<'w, 's, Q, F>,
//     archetypes: Vec<&'w Archetype>,
//     state: Option<Q::State<'w>>,
//     filter: Option<F::State<'w>>,
//     entities: Option<indexmap::set::Iter<'w, Entity>>,
//     archetype: usize,
// }

// impl<'w, 's, Q: BaseQuery, F: BaseFilter> QueryIter<'w, 's, Q, F> {
//     pub fn new(query: &'w Query<'w, 's, Q, F>) -> Self {
//         let world = unsafe { query.world.get() };
//         let archetypes = world.archetypes().query(&query.state.query);

//         let (state, filter_state, entities) = archetypes
//             .get(0)
//             .map(|archetype| {
//                 let state = Q::state(
//                     &query.state.data,
//                     query.world,
//                     archetype,
//                     query.current_frame,
//                     query.system_frame,
//                 );
//                 let filter_state = F::state(
//                     &query.state.filter,
//                     query.world,
//                     archetype,
//                     query.current_frame,
//                     query.system_frame,
//                 );

//                 let entities = archetype.table().entities();

//                 (Some(state), Some(filter_state), Some(entities))
//             })
//             .unwrap_or((None, None, None));

//         Self {
//             query,
//             archetypes,
//             state,
//             filter: filter_state,
//             entities,
//             archetype: 0,
//         }
//     }

//     pub fn contains(&self, entity: Entity) -> bool {
//         self.query.contains(entity)
//     }
// }

// impl<'w, 's, Q: BaseQuery, F: BaseFilter> Iterator for QueryIter<'w, 's, Q, F> {
//     type Item = Q::Item<'w>;

//     fn next(&mut self) -> Option<Self::Item> {
//         if self.archetype >= self.archetypes.len() {
//             None
//         } else if let Some(entity) = self
//             .entities
//             .as_mut()
//             .and_then(|entities| entities.next())
//             .copied()
//         {
//             let row = self.archetypes[self.archetype]
//                 .table()
//                 .get_entity_row(entity)
//                 .unwrap();

//             let state = self.state.as_mut()?;
//             let filter = match &mut self.filter {
//                 Some(state) => F::filter(state, entity, row),
//                 None => true,
//             };

//             filter.then_some(Q::get(state, entity, row))
//         } else {
//             self.archetype += 1;
//             self.entities = self.archetypes.get(self.archetype).map(|archetype| {
//                 self.state = Some(Q::state(
//                     &self.query.state.data,
//                     self.query.world,
//                     archetype,
//                     self.query.current_frame,
//                     self.query.system_frame,
//                 ));
//                 self.filter = Some(F::state(
//                     &self.query.state.filter,
//                     self.query.world,
//                     archetype,
//                     self.query.current_frame,
//                     self.query.system_frame,
//                 ));
//                 archetype.table().entities()
//             });

//             self.next()
//         }
//     }
// }

// #[macro_export]
// macro_rules! impl_base_query_for_tuples {
//     ($(($($name:ident),*)),*)  => {
//         $(
//             #[allow(non_snake_case)]
//             impl<$($name: BaseQuery),+> BaseQuery for ($($name),+) {
//                 type Item<'w> = ($($name::Item<'w>), +);

//                 type State<'w> = ($($name::State<'w>), +);

//                 type Data = ($($name::Data), +);

//                 fn init(builder: &mut QueryBuilder) -> Self::Data {
//                     ($($name::init(builder),)*)
//                 }

//                 fn state<'w>(data: &'w Self::Data, world: WorldCell<'w>, archetype: &'w Archetype, current_frame: Frame, system_frame: Frame) -> Self::State<'w> {
//                     let ($($name,)*) = data;
//                     ($($name::state($name, world, archetype, current_frame, system_frame),)*)
//                 }

//                 fn get<'w>(state: &mut Self::State<'w>, entity: Entity, row: RowIndex) -> Self::Item<'w> {
//                     let ($($name,)*) = state;

//                     ($(
//                         $name::get($name, entity, row),
//                     )*)
//                 }
//             }

//             #[allow(non_snake_case)]
//             impl<$($name: BaseFilter),+> BaseFilter for ($($name),+) {
//                 type State<'w> = ($($name::State<'w>), +);

//                 type Data = ($($name::Data), +);

//                 fn init(builder: &mut QueryBuilder) -> Self::Data {
//                     ($($name::init(builder),)*)
//                 }

//                 fn state<'w>(data: &Self::Data, world: WorldCell<'w>, archetype: &'w Archetype, current_frame: Frame, system_frame: Frame) -> Self::State<'w> {
//                     let ($($name,)*) = data;
//                     ($($name::state($name, world, archetype, current_frame, system_frame),)*)
//                 }

//                 fn filter<'w>(state: &Self::State<'w>, entity: Entity, row: RowIndex) -> bool {
//                     let ($($name,)*) = state;
//                     let mut filter = true;

//                     ($(
//                         filter = filter && $name::filter($name, entity, row),
//                     )*);

//                     filter
//                 }
//             }

//             #[allow(non_snake_case)]
//             impl<$($name: BaseFilter),+> BaseFilter for Or<'_, ($($name),+)> {
//                 type State<'w> = ($($name::State<'w>), +);

//                 type Data = ($($name::Data), +);

//                 fn init(builder: &mut QueryBuilder) -> Self::Data {
//                     ($($name::init(builder),)*)
//                 }

//                 fn state<'w>(data: &Self::Data, world: WorldCell<'w>, archetype: &'w Archetype, current_frame: Frame, system_frame: Frame) -> Self::State<'w> {
//                     let ($($name,)*) = data;
//                     ($($name::state($name, world, archetype, current_frame, system_frame),)*)
//                 }

//                 fn filter<'w>(state: &Self::State<'w>, entity: Entity, row: RowIndex) -> bool {
//                     let ($($name,)*) = state;
//                     let mut filter = true;

//                     ($(
//                         filter = filter || $name::filter($name, entity, row),
//                     )*);

//                     filter
//                 }
//             }

//             unsafe impl<$($name: ReadQuery),+> ReadQuery for ($($name),+) {}
//         )+
//     };
// }

// impl_base_query_for_tuples!((A, B));
// impl_base_query_for_tuples!((A, B, C));
// impl_base_query_for_tuples!((A, B, C, D));
// impl_base_query_for_tuples!((A, B, C, D, E));
// impl_base_query_for_tuples!((A, B, C, D, E, F));
// impl_base_query_for_tuples!((A, B, C, D, E, F, G));
// impl_base_query_for_tuples!((A, B, C, D, E, F, G, H));
// impl_base_query_for_tuples!((A, B, C, D, E, F, G, H, I));
// impl_base_query_for_tuples!((A, B, C, D, E, F, G, H, I, J));
// impl_base_query_for_tuples!((A, B, C, D, E, F, G, H, I, J, K));
// impl_base_query_for_tuples!((A, B, C, D, E, F, G, H, I, J, K, L));
// impl_base_query_for_tuples!((A, B, C, D, E, F, G, H, I, J, K, L, M));
// impl_base_query_for_tuples!((A, B, C, D, E, F, G, H, I, J, K, L, M, N));
// impl_base_query_for_tuples!((A, B, C, D, E, F, G, H, I, J, K, L, M, N, O));
// impl_base_query_for_tuples!((A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P));
// impl_base_query_for_tuples!((A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q));

// #[cfg(test)]
// mod tests {

//     use crate::system::SystemMeta;

//     use super::*;

//     #[derive(Debug, PartialEq, Eq)]
//     struct Age(u32);
//     impl Component for Age {}

//     #[derive(Debug, PartialEq, Eq)]
//     struct Name(&'static str);
//     impl Component for Name {}

//     #[test]
//     fn test_query() {
//         let mut world = World::new();
//         world.register::<Age>();
//         world.register::<Name>();

//         let entity = world.spawn();
//         world.add_component(entity, Age(0));
//         world.add_component(entity, Name("Bob"));

//         let mut system = SystemInit::new(&mut world, None);
//         let state = QueryState::<(&Age, &Name)>::new(&mut system);
//         let query = Query::new(&world, &state);
//         let iter = query.iter();

//         assert!(iter.contains(entity));
//     }

//     #[test]
//     fn test_with_query() {
//         let mut world = World::new();
//         world.register::<Age>();
//         world.register::<Name>();

//         let entity = world.spawn();
//         world.add_component(entity, Age(0));
//         world.add_component(entity, Name("Bob"));

//         let mut system = SystemInit::new(&mut world, None);
//         let state = QueryState::<&Age, With<Name>>::new(&mut system);
//         let query = Query::new(&world, &state);

//         assert!(query.contains(entity));
//     }

//     #[test]
//     fn test_without_query() {
//         let mut world = World::new();
//         world.register::<Age>();
//         world.register::<Name>();

//         let entity = world.spawn();
//         world.add_component(entity, Age(0));
//         world.add_component(entity, Name("Bob"));

//         let mut system = SystemInit::new(&mut world, None);
//         let state = QueryState::<&Age, Without<Name>>::new(&mut system);
//         let query = Query::new(&world, &state);

//         assert!(!query.contains(entity));
//     }

//     #[test]
//     fn test_added_query() {
//         let mut world = World::new();
//         world.register::<Age>();
//         world.register::<Name>();

//         let entity = world.spawn();
//         world.add_component(entity, Age(0));
//         world.add_component(entity, Name("Bob"));

//         let mut system = SystemInit::new(&mut world, None);
//         let mut state = Query::<&Age, Added<Name>>::init(&mut system);
//         let system = SystemMeta::default();
//         let query = unsafe { Query::get(&mut state, world.cell(), &system) };

//         assert!(query.contains(entity))
//     }

//     #[test]
//     fn test_not_query() {
//         let mut world = World::new();
//         world.register::<Age>();
//         world.register::<Name>();

//         let entity_a = world.spawn();
//         world.add_component(entity_a, Age(0));
//         world.add_component(entity_a, Name("Bob"));

//         let entity_b = world.spawn();
//         world.add_component(entity_b, Age(1));

//         let mut system = SystemInit::new(&mut world, None);
//         let state = QueryState::<&Age, Not<Added<Name>>>::new(&mut system);
//         let query = Query::new(&world, &state);

//         assert!(!query.contains(entity_a));
//         assert!(query.contains(entity_b));
//     }

//     #[test]
//     fn test_or_query() {
//         let mut world = World::new();
//         world.register::<Age>();
//         world.register::<Name>();

//         let entity_a = world.spawn();
//         let entity_b = world.spawn();
//         let entity_c = world.spawn();
//         world.add_component(entity_a, Age(32));
//         world.add_component(entity_b, Name("Bob"));

//         let mut system = SystemInit::new(&mut world, None);
//         let state = QueryState::<Entity, Or<(With<Age>, With<Name>)>>::new(&mut system);
//         let query = Query::new(&world, &state);

//         assert!(query.contains(entity_a));
//         assert!(query.contains(entity_b));
//         assert!(!query.contains(entity_c));
//     }
// }
