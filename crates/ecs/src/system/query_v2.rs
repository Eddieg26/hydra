use crate::{
    Archetype, ArchetypeAccess, ArchetypeId, Column, Component, ComponentId, Entity, Frame,
    ObjectStatus, Ptr, RowIndex, SparseIndex, World, WorldCell,
};

use super::FullAccess;

pub trait BaseQuery {
    type Item<'w>;
    type State<'w>;

    /// Data used to construct the state of the query.
    /// This is used to create the query state when the query is first created.
    type Data: Send + Sync + Sized;

    fn init(world: &mut World, access: &mut FullAccess<ComponentId>) -> Self::Data;

    fn state<'w>(
        data: &'w Self::Data,
        world: WorldCell<'w>,
        archetype: &'w Archetype,
        current_frame: Frame,
        system_frame: Frame,
    ) -> Self::State<'w>;

    fn get<'w>(state: &mut Self::State<'w>, entity: Entity, row: RowIndex) -> Self::Item<'w>;
}

pub trait BaseFilter {
    type State<'w>;
    type Data: Send + Sync + Sized;

    fn init(world: &mut World, access: &mut FullAccess<ComponentId>) -> Self::Data;

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

    fn init(_: &mut World, _: &mut FullAccess<ComponentId>) -> Self::Data {
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

impl BaseFilter for () {
    type State<'w> = ();

    type Data = ();

    fn init(_: &mut World, _: &mut FullAccess<ComponentId>) -> Self::Data {
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

    fn init(_: &mut World, _: &mut FullAccess<ComponentId>) -> Self::Data {
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

    fn init(world: &mut World, access: &mut FullAccess<ComponentId>) -> Self::Data {
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

pub struct WritePtr<'w, C: Component> {
    components: Ptr<'w, C>,
    frames: Ptr<'w, ObjectStatus>,
    current_frame: Frame,
    system_frame: Frame,
}

impl<'w, C: Component> WritePtr<'w, C> {
    pub fn new(
        components: Ptr<'w, C>,
        frames: Ptr<'w, ObjectStatus>,
        current_frame: Frame,
        system_frame: Frame,
    ) -> Self {
        Self {
            components,
            frames,
            current_frame,
            system_frame,
        }
    }
}

impl<C: Component> BaseQuery for &mut C {
    type Item<'w> = &'w mut C;

    type State<'w> = WritePtr<'w, C>;

    type Data = ComponentId;

    fn init(world: &mut World, access: &mut FullAccess<ComponentId>) -> Self::Data {
        access.write(world.register::<C>())
    }

    fn state<'w>(
        data: &'w Self::Data,
        _: WorldCell<'w>,
        archetype: &'w Archetype,
        current_frame: Frame,
        system_frame: Frame,
    ) -> Self::State<'w> {
        match archetype.table().get_column(*data) {
            Some(column) => {
                let (components, frames) = unsafe { column.get_ptr::<C>() };
                WritePtr::new(components, frames, current_frame, system_frame)
            }
            None => {
                let component = std::any::type_name::<C>();
                panic!("Component does not exist in this archetype: {}", component);
            }
        }
    }

    fn get<'w>(state: &mut Self::State<'w>, _: Entity, row: RowIndex) -> Self::Item<'w> {
        unsafe {
            state.frames.get_mut(row.to_usize()).unwrap().added = state.current_frame;
            state.components.get_mut(row.to_usize()).unwrap()
        }
    }
}

impl<C: Component> BaseQuery for Option<&C> {
    type Item<'w> = Option<<&'w C as BaseQuery>::Item<'w>>;

    type State<'w> = Option<<&'w C as BaseQuery>::State<'w>>;

    type Data = <&'static C as BaseQuery>::Data;

    fn init(world: &mut World, access: &mut FullAccess<ComponentId>) -> Self::Data {
        <&C as BaseQuery>::init(world, access)
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

impl<C: Component> BaseQuery for Option<&mut C> {
    type Item<'w> = Option<<&'w mut C as BaseQuery>::Item<'w>>;

    type State<'w> = Option<<&'w mut C as BaseQuery>::State<'w>>;

    type Data = <&'static mut C as BaseQuery>::Data;

    fn init(world: &mut World, access: &mut FullAccess<ComponentId>) -> Self::Data {
        <&mut C as BaseQuery>::init(world, access)
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

    fn init(world: &mut World, access: &mut FullAccess<ComponentId>) -> Self::Data {
        access.include(world.register::<C>())
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

    fn init(world: &mut World, access: &mut FullAccess<ComponentId>) -> Self::Data {
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

pub struct Added<C: Component>(std::marker::PhantomData<C>);
impl<C: Component> BaseFilter for Added<C> {
    type State<'w> = ReadPtr<'w, C>;

    type Data = ComponentId;

    fn init(world: &mut World, access: &mut FullAccess<ComponentId>) -> Self::Data {
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

pub struct Modified<C: Component>(std::marker::PhantomData<C>);
impl<C: Component> BaseFilter for Modified<C> {
    type State<'w> = ReadPtr<'w, C>;

    type Data = ComponentId;

    fn init(world: &mut World, access: &mut FullAccess<ComponentId>) -> Self::Data {
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

pub struct QueryState<Q: BaseQuery, F: BaseFilter = ()> {
    access: ArchetypeAccess,
    archetypes: Vec<ArchetypeId>,
    last_archetype: usize,
    data: Q::Data,
    filter: F::Data,
}

impl<Q: BaseQuery, F: BaseFilter> QueryState<Q, F> {
    pub fn update(&mut self, world: &mut World) {
        let archetypes = world.archetypes.archetypes();
        self.last_archetype = if self.last_archetype >= archetypes.len() {
            self.archetypes.clear();
            0
        } else {
            self.last_archetype
        };

        for archetype in &archetypes[self.last_archetype..] {
            if archetype.matches(&self.access) {
                self.archetypes.push(archetype.id());
            }
        }
    }
}

pub struct Query<'w, 's, Q: BaseQuery, F: BaseFilter = ()> {
    world: WorldCell<'w>,
    state: &'s QueryState<Q, F>,
    current_frame: Frame,
    system_frame: Frame,
}
