use super::{IntoSystemConfig, SystemConfig, SystemMeta, SystemType, SystemSet};
use crate::{
    Event, EventReader, EventWriter, Events, WorldAccess,
    world::{
        Cloned, Entities, EventStorage, NonSend, NonSendMut, Resource, ResourceId, World, WorldCell,
    },
};
use derive_ecs::Resource;
use std::any::Any;

#[allow(unused_variables)]
pub unsafe trait SystemArg: Sized {
    type Item<'world, 'state>: SystemArg<State = Self::State>;

    type State: Send + Sync + 'static;

    fn init(world: &mut World, access: &mut WorldAccess) -> Self::State;

    /// Validates that the argument can be accessed by the system
    unsafe fn validate(state: &Self::State, world: WorldCell, system: &SystemMeta) -> bool {
        true
    }

    unsafe fn get<'world, 'state>(
        state: &'state mut Self::State,
        world: WorldCell<'world>,
        system: &'world SystemMeta,
    ) -> Self::Item<'world, 'state>;

    fn exclusive() -> bool {
        false
    }

    fn send() -> bool {
        true
    }

    fn update(state: &mut Self::State, world: &mut World) {}
}

pub unsafe trait ReadOnly: SystemArg {}

pub type ArgItem<'world, 'state, A> = <A as SystemArg>::Item<'world, 'state>;

unsafe impl SystemArg for () {
    type Item<'world, 'state> = ();

    type State = ();

    fn init(_: &mut World, _: &mut WorldAccess) -> Self::State {
        ()
    }

    unsafe fn get<'world, 'state>(
        _state: &'state mut Self::State,
        _world: WorldCell<'world>,
        _system: &'world SystemMeta,
    ) -> Self::Item<'world, 'state> {
        ()
    }
}

unsafe impl ReadOnly for () {}

unsafe impl SystemArg for &World {
    type Item<'world, 'state> = &'world World;

    type State = ();

    fn init(_: &mut World, _: &mut WorldAccess) -> Self::State {
        ()
    }

    unsafe fn get<'world, 'state>(
        _state: &'state mut Self::State,
        world: WorldCell<'world>,
        _system: &'world SystemMeta,
    ) -> Self::Item<'world, 'state> {
        unsafe { world.get() }
    }

    fn exclusive() -> bool {
        true
    }
}

unsafe impl SystemArg for &mut World {
    type Item<'world, 'state> = &'world mut World;

    type State = ();

    fn init(_: &mut World, _: &mut WorldAccess) -> Self::State {
        ()
    }

    unsafe fn get<'world, 'state>(
        _state: &'state mut Self::State,
        mut world: WorldCell<'world>,
        _system: &'world SystemMeta,
    ) -> Self::Item<'world, 'state> {
        unsafe { world.get_mut() }
    }

    fn exclusive() -> bool {
        true
    }
}

unsafe impl ReadOnly for &World {}

unsafe impl SystemArg for &Entities {
    type Item<'world, 'state> = &'world Entities;

    type State = ();

    fn init(_: &mut World, _: &mut WorldAccess) -> Self::State {
        ()
    }

    unsafe fn get<'world, 'state>(
        _state: &'state mut Self::State,
        world: WorldCell<'world>,
        _system: &'world SystemMeta,
    ) -> Self::Item<'world, 'state> {
        unsafe { world.get().entities() }
    }
}

unsafe impl ReadOnly for &Entities {}

unsafe impl<R: Resource + Send> SystemArg for &R {
    type Item<'world, 'state> = &'world R;

    type State = ResourceId;

    fn init(world: &mut World, access: &mut WorldAccess) -> Self::State {
        let id = world.register_resource::<R>();
        access.resources_mut().read(id);
        id
    }

    unsafe fn validate(_: &Self::State, world: WorldCell, _: &SystemMeta) -> bool {
        unsafe { world.get() }.resources.contains::<R>()
    }

    unsafe fn get<'world, 'state>(
        state: &'state mut Self::State,
        world: WorldCell<'world>,
        system: &'world SystemMeta,
    ) -> Self::Item<'world, 'state> {
        unsafe { world.get() }
            .resources()
            .get::<R>(*state)
            .expect(&format!(
                "Resource of type {} not found for system: {:?}",
                std::any::type_name::<R>(),
                system.name,
            ))
    }
}

unsafe impl<R: Resource + Send> ReadOnly for &R {}

unsafe impl<R: Resource + Send> SystemArg for &mut R {
    type Item<'world, 'state> = &'world mut R;

    type State = ResourceId;

    fn init(world: &mut World, access: &mut WorldAccess) -> Self::State {
        let id = world.register_resource::<R>();
        access.resources_mut().write(id);
        id
    }

    unsafe fn validate(_: &Self::State, world: WorldCell, _: &SystemMeta) -> bool {
        unsafe { world.get() }.resources.contains::<R>()
    }

    unsafe fn get<'world, 'state>(
        state: &'state mut Self::State,
        mut world: WorldCell<'world>,
        system: &'world SystemMeta,
    ) -> Self::Item<'world, 'state> {
        let world = unsafe { world.get_mut() };
        world
            .resources
            .get_mut::<R>(*state, world.frame)
            .expect(&format!(
                "Resource of type {} not found for system: {:?}",
                std::any::type_name::<R>(),
                system.name,
            ))
    }
}

unsafe impl<R: Resource> ReadOnly for NonSend<'_, R> {}

unsafe impl<R: Resource> SystemArg for NonSend<'_, R> {
    type Item<'world, 'state> = NonSend<'world, R>;

    type State = ResourceId;

    fn init(world: &mut World, access: &mut WorldAccess) -> Self::State {
        let id = world.register_non_send_resource::<R>();
        access.resources_mut().read(id);
        id
    }

    unsafe fn validate(_: &Self::State, world: WorldCell, _: &SystemMeta) -> bool {
        unsafe { world.get() }.resources.contains::<R>()
    }

    unsafe fn get<'world, 'state>(
        state: &'state mut Self::State,
        world: WorldCell<'world>,
        system: &'world SystemMeta,
    ) -> Self::Item<'world, 'state> {
        let resource = unsafe { world.get() }
            .resources()
            .get::<R>(*state)
            .expect(&format!(
                "Resource of type {} not found for system: {:?}",
                std::any::type_name::<R>(),
                system.name,
            ));

        NonSend::new(resource)
    }

    fn send() -> bool {
        false
    }
}

unsafe impl<R: Resource> SystemArg for NonSendMut<'_, R> {
    type Item<'world, 'state> = NonSendMut<'world, R>;

    type State = ResourceId;

    fn init(world: &mut World, access: &mut WorldAccess) -> Self::State {
        let id = world.register_non_send_resource::<R>();
        access.resources_mut().write(id);
        id
    }

    unsafe fn validate(_: &Self::State, world: WorldCell, _: &SystemMeta) -> bool {
        unsafe { world.get() }.resources.contains::<R>()
    }

    unsafe fn get<'world, 'state>(
        state: &'state mut Self::State,
        mut world: WorldCell<'world>,
        system: &'world SystemMeta,
    ) -> Self::Item<'world, 'state> {
        let world = unsafe { world.get_mut() };
        let resource = world
            .resources
            .get_mut::<R>(*state, world.frame)
            .expect(&format!(
                "NonSendMut resource of type {} not found for system {:?}",
                std::any::type_name::<R>(),
                system.name
            ));

        NonSendMut::new(resource)
    }

    fn send() -> bool {
        false
    }
}

unsafe impl<R: Resource + Send + Clone> SystemArg for Cloned<R> {
    type Item<'world, 'state> = Cloned<R>;

    type State = ResourceId;

    fn init(world: &mut World, _: &mut WorldAccess) -> Self::State {
        world.register_resource::<R>()
    }

    unsafe fn validate(_: &Self::State, world: WorldCell, _: &SystemMeta) -> bool {
        unsafe { world.get() }.resources.contains::<R>()
    }

    unsafe fn get<'world, 'state>(
        state: &'state mut Self::State,
        world: WorldCell<'world>,
        system: &'world SystemMeta,
    ) -> Self::Item<'world, 'state> {
        let resource = unsafe { world.get() }
            .resources()
            .get::<R>(*state)
            .cloned()
            .expect(&format!(
                "Cloned resource of type {} not found for system {:?}",
                std::any::type_name::<R>(),
                system.name
            ));

        Cloned::new(resource)
    }
}

unsafe impl<R: Resource + Send> SystemArg for Option<&R> {
    type Item<'world, 'state> = Option<&'world R>;

    type State = ResourceId;

    fn init(world: &mut World, access: &mut WorldAccess) -> Self::State {
        <&R as SystemArg>::init(world, access)
    }

    unsafe fn validate(state: &Self::State, world: WorldCell, system: &SystemMeta) -> bool {
        unsafe { <&R as SystemArg>::validate(state, world, system) }
    }

    unsafe fn get<'world, 'state>(
        state: &'state mut Self::State,
        world: WorldCell<'world>,
        system: &'world SystemMeta,
    ) -> Self::Item<'world, 'state> {
        unsafe {
            if <&R as SystemArg>::validate(state, world, system) {
                Some(<&R as SystemArg>::get(state, world, system))
            } else {
                None
            }
        }
    }

    fn send() -> bool {
        true
    }

    fn exclusive() -> bool {
        false
    }

    fn update(state: &mut Self::State, world: &mut World) {
        <&R as SystemArg>::update(state, world);
    }
}

unsafe impl<R: Resource + Send> SystemArg for Option<&mut R> {
    type Item<'world, 'state> = Option<&'world R>;

    type State = ResourceId;

    fn init(world: &mut World, access: &mut WorldAccess) -> Self::State {
        <&mut R as SystemArg>::init(world, access)
    }

    unsafe fn validate(state: &Self::State, world: WorldCell, system: &SystemMeta) -> bool {
        unsafe { <&mut R as SystemArg>::validate(state, world, system) }
    }

    unsafe fn get<'world, 'state>(
        state: &'state mut Self::State,
        world: WorldCell<'world>,
        system: &'world SystemMeta,
    ) -> Self::Item<'world, 'state> {
        unsafe {
            if <&mut R as SystemArg>::validate(state, world, system) {
                Some(<&mut R as SystemArg>::get(state, world, system))
            } else {
                None
            }
        }
    }

    fn send() -> bool {
        true
    }

    fn exclusive() -> bool {
        false
    }

    fn update(state: &mut Self::State, world: &mut World) {
        <&mut R as SystemArg>::update(state, world);
    }
}

unsafe impl<R: Resource> SystemArg for Option<NonSend<'_, R>> {
    type Item<'world, 'state> = Option<NonSend<'world, R>>;

    type State = ResourceId;

    fn init(world: &mut World, access: &mut WorldAccess) -> Self::State {
        <NonSend<R> as SystemArg>::init(world, access)
    }

    unsafe fn validate(state: &Self::State, world: WorldCell, system: &SystemMeta) -> bool {
        unsafe { <NonSend<R> as SystemArg>::validate(state, world, system) }
    }

    unsafe fn get<'world, 'state>(
        state: &'state mut Self::State,
        world: WorldCell<'world>,
        system: &'world SystemMeta,
    ) -> Self::Item<'world, 'state> {
        unsafe {
            if <NonSend<R> as SystemArg>::validate(state, world, system) {
                Some(<NonSend<R> as SystemArg>::get(state, world, system))
            } else {
                None
            }
        }
    }

    fn send() -> bool {
        false
    }

    fn exclusive() -> bool {
        false
    }

    fn update(state: &mut Self::State, world: &mut World) {
        <NonSend<R> as SystemArg>::update(state, world);
    }
}

unsafe impl<R: Resource> SystemArg for Option<NonSendMut<'_, R>> {
    type Item<'world, 'state> = Option<NonSendMut<'world, R>>;

    type State = ResourceId;

    fn init(world: &mut World, access: &mut WorldAccess) -> Self::State {
        <NonSendMut<R> as SystemArg>::init(world, access)
    }

    unsafe fn validate(state: &Self::State, world: WorldCell, system: &SystemMeta) -> bool {
        unsafe { <NonSendMut<R> as SystemArg>::validate(state, world, system) }
    }

    unsafe fn get<'world, 'state>(
        state: &'state mut Self::State,
        world: WorldCell<'world>,
        system: &'world SystemMeta,
    ) -> Self::Item<'world, 'state> {
        unsafe {
            if <NonSendMut<R> as SystemArg>::validate(state, world, system) {
                Some(<NonSendMut<R> as SystemArg>::get(state, world, system))
            } else {
                None
            }
        }
    }

    fn send() -> bool {
        false
    }

    fn exclusive() -> bool {
        false
    }

    fn update(state: &mut Self::State, world: &mut World) {
        <NonSendMut<R> as SystemArg>::update(state, world);
    }
}

unsafe impl<R: Resource + Send> ReadOnly for Option<&R> {}
unsafe impl<R: Resource> ReadOnly for Option<NonSend<'_, R>> {}

unsafe impl<E: Event> SystemArg for EventReader<'_, E> {
    type Item<'world, 'state> = EventReader<'world, E>;

    type State = ();

    fn init(world: &mut World, _: &mut crate::WorldAccess) -> Self::State {
        world.register_event::<E>();
        ()
    }

    unsafe fn get<'world, 'state>(
        _: &'state mut Self::State,
        world: super::WorldCell<'world>,
        _: &'world SystemMeta,
    ) -> Self::Item<'world, 'state> {
        let events = unsafe { world.get().resource::<Events<E>>() };
        EventReader::new(events)
    }
}

unsafe impl<E: Event> ReadOnly for EventReader<'_, E> {}

unsafe impl<E: Event> SystemArg for EventWriter<'_, E> {
    type Item<'world, 'state> = EventWriter<'state, E>;

    type State = EventStorage<E>;

    fn init(world: &mut World, _: &mut crate::WorldAccess) -> Self::State {
        world.register_event::<E>();
        EventStorage::default()
    }

    unsafe fn get<'world, 'state>(
        state: &'state mut Self::State,
        _: super::WorldCell<'world>,
        _system: &'world SystemMeta,
    ) -> Self::Item<'world, 'state> {
        EventWriter::new(state)
    }

    fn update(state: &mut Self::State, world: &mut super::World) {
        let events = world.resource_mut::<Events<E>>();
        let offset = events.write.events.len();
        events.write.events.append(&mut state.events);
        for (entity, added) in state.entities.drain() {
            let indices = events.write.entities.entry(entity).or_default();
            indices.extend(added.iter().map(|i| *i + offset));
        }
    }
}

unsafe impl SystemArg for &SystemMeta {
    type Item<'world, 'state> = &'world SystemMeta;

    type State = ();

    fn init(_: &mut World, _: &mut WorldAccess) -> Self::State {
        ()
    }

    unsafe fn get<'world, 'state>(
        _: &'state mut Self::State,
        _: WorldCell<'world>,
        system: &'world SystemMeta,
    ) -> Self::Item<'world, 'state> {
        system
    }
}

unsafe impl ReadOnly for &SystemMeta {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Resource)]
pub struct MainWorld(WorldCell<'static>);
impl MainWorld {
    pub(crate) fn new(world: &mut World) -> Self {
        let cell: WorldCell<'static> = unsafe { std::mem::transmute(world.cell()) };
        Self(cell)
    }
}

impl std::ops::Deref for MainWorld {
    type Target = World;

    fn deref(&self) -> &Self::Target {
        unsafe { self.0.get() }
    }
}

impl std::ops::DerefMut for MainWorld {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { self.0.get_mut() }
    }
}

unsafe impl Send for MainWorld {}
unsafe impl Sync for MainWorld {}

pub struct Main<'w, 's, S: SystemArg>(ArgItem<'w, 's, S>);
impl<'w, 's, S: SystemArg> Main<'w, 's, S> {
    pub fn new(arg: ArgItem<'w, 's, S>) -> Self {
        Self(arg)
    }

    pub fn into_inner(self) -> ArgItem<'w, 's, S> {
        self.0
    }
}

impl<'w, 's, S: SystemArg> std::ops::Deref for Main<'w, 's, S> {
    type Target = ArgItem<'w, 's, S>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<'w, 's, S: SystemArg> std::ops::DerefMut for Main<'w, 's, S> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

unsafe impl<S: SystemArg> SystemArg for Main<'_, '_, S> {
    type Item<'w, 's> = Main<'w, 's, S>;
    type State = S::State;

    fn init(world: &mut World, access: &mut crate::WorldAccess) -> Self::State {
        let mut main_access = WorldAccess::from(std::mem::take(access.main_mut()));
        let main = world.resource_mut::<MainWorld>();
        let state = S::init(main, &mut main_access);

        access.main = main_access.current;

        state
    }

    unsafe fn get<'world, 'state>(
        state: &'state mut Self::State,
        mut world: WorldCell<'world>,
        system: &'world crate::SystemMeta,
    ) -> Self::Item<'world, 'state> {
        let main = unsafe { world.get_mut().resource_mut::<MainWorld>().cell() };
        let arg = unsafe { S::get(state, main, system) };
        Main(arg)
    }

    fn update(state: &mut Self::State, world: &mut World) {
        let main = world.resource_mut::<MainWorld>();
        S::update(state, main);
    }

    unsafe fn validate(
        state: &Self::State,
        mut world: WorldCell,
        system: &crate::SystemMeta,
    ) -> bool {
        unsafe {
            let main = world.get_mut().resource_mut::<MainWorld>().cell();
            S::validate(state, main, system)
        }
    }

    fn exclusive() -> bool {
        S::exclusive()
    }

    fn send() -> bool {
        S::send()
    }
}

macro_rules! impl_into_system_configs {
    ($($arg:ident),*) => {
    #[allow(non_snake_case)]
    impl<O, F, $($arg: SystemArg),*> IntoSystemConfig<(F, $($arg),*)> for F
        where
            for<'world, 'state> F: Fn($($arg),*) + Fn($(ArgItem<'world,'state, $arg>),*) -> O + Send + Sync + 'static,
        {
            fn config(self) -> SystemConfig {
                let id = SystemType::<F>::new().identify();
                let name = std::any::type_name::<F>();

                let init = |world: &mut World, access: &mut WorldAccess| {
                    let ($($arg,)*) = ($({
                        let arg_state = $arg::init(world, access);
                        arg_state
                    },)*);

                    let state = ($($arg,)*);
                    Box::new(state) as Box<dyn Any + Send + Sync>
                };

                let run = move |state: &mut Box<dyn Any + Send + Sync>, world: WorldCell, system: &SystemMeta| {
                    let ($($arg,)*) = state.downcast_mut::<($($arg::State,)*)>().unwrap();
                    let ($($arg,)*) = unsafe {($($arg::get($arg, world, system),)*)};

                    self($($arg,)*);
                };

                let update = |state: &mut Box<dyn Any + Send + Sync>, world: &mut World| {
                    let ($($arg,)*) = state.downcast_mut::<($($arg::State,)*)>().unwrap();
                    $($arg::update($arg, world);)*
                };

                let send = ($($arg::send() &&)* true);
                let exclusive = ($($arg::exclusive() ||)* false);

                SystemConfig::new(id, name, exclusive, send, init, update, Box::new(run), |_, _| true)
            }
        }

        #[allow(non_snake_case)]
        unsafe impl<$($arg: SystemArg),*> SystemArg for ($($arg,)*) {
            type Item<'world, 'state> = ($($arg::Item<'world, 'state>,)*);
            type State = ($($arg::State,)*);

            fn init(world: &mut World, access: &mut WorldAccess) -> Self::State {
                let ($($arg,)*) = ($({
                    let arg_state = $arg::init(world, access);
                    arg_state
                },)*);

                ($($arg,)*)
            }

            unsafe fn get<'world, 'state>(state: &'state mut Self::State, world: WorldCell<'world>, system: &'world SystemMeta,) -> Self::Item<'world, 'state> {
                let ($($arg,)*) = state;
                let ($($arg,)*) = unsafe {($($arg::get($arg, world, system),)*)};
                ($($arg,)*)
            }

             fn exclusive() -> bool {
                ($($arg::exclusive() ||)* false)
            }

            fn send() -> bool {
                ($($arg::send() &&)* true)
            }

            fn update(state: &mut Self::State, world: &mut World) {
                let ($($arg,)*) = state;
                $($arg::update($arg, world);)*
            }
        }
    };
}

variadics::variable_impl!(impl_into_system_configs, P, 16);

macro_rules! impl_read_only_for_tuples {
    ($($arg:ident), *) => {
        #[allow(non_snake_case)]
        unsafe impl<$($arg: ReadOnly),*> ReadOnly for ($($arg),*) {}
    };
}

variadics::variable_impl!(impl_read_only_for_tuples, P, 2, 16);

#[allow(type_alias_bounds)]
pub mod unlifetime {
    use super::{ArgItem, SystemArg};
    use crate::{BaseFilter, BaseQuery, Commands, NonSend, NonSendMut, Query, Resource};

    pub type Read<T> = &'static T;
    pub type Write<T> = &'static mut T;
    pub type NonSendRes<R: Resource> = NonSend<'static, R>;
    pub type NonSendResMut<R: Resource> = NonSendMut<'static, R>;
    pub type SQuery<Q: BaseQuery, F: BaseFilter = ()> = Query<'static, 'static, Q, F>;
    pub type SCommands = Commands<'static, 'static>;

    pub struct StaticArg<'w, 's, S: SystemArg>(ArgItem<'w, 's, S>);
    impl<'w, 's, S: SystemArg> StaticArg<'w, 's, S> {
        pub fn get(&self) -> &ArgItem<'w, 's, S> {
            &self.0
        }

        pub fn get_mut(&mut self) -> &mut ArgItem<'w, 's, S> {
            &mut self.0
        }

        pub fn into_inner(self) -> ArgItem<'w, 's, S> {
            self.0
        }
    }

    impl<'w, 's, S: SystemArg> std::ops::Deref for StaticArg<'w, 's, S> {
        type Target = ArgItem<'w, 's, S>;

        fn deref(&self) -> &Self::Target {
            &self.0
        }
    }

    impl<'w, 's, S: SystemArg> std::ops::DerefMut for StaticArg<'w, 's, S> {
        fn deref_mut(&mut self) -> &mut Self::Target {
            &mut self.0
        }
    }

    unsafe impl<S: SystemArg + 'static> SystemArg for StaticArg<'_, '_, S> {
        type Item<'world, 'state> = StaticArg<'world, 'state, S>;

        type State = S::State;

        fn init(world: &mut crate::World, access: &mut crate::WorldAccess) -> Self::State {
            S::init(world, access)
        }

        unsafe fn get<'world, 'state>(
            state: &'state mut Self::State,
            world: crate::world::WorldCell<'world>,
            system: &'world crate::SystemMeta,
        ) -> Self::Item<'world, 'state> {
            unsafe { StaticArg(S::get(state, world, system)) }
        }

        unsafe fn validate(
            state: &Self::State,
            world: crate::world::WorldCell,
            system: &crate::SystemMeta,
        ) -> bool {
            unsafe { S::validate(state, world, system) }
        }

        fn exclusive() -> bool {
            S::exclusive()
        }

        fn send() -> bool {
            S::send()
        }

        fn update(state: &mut Self::State, world: &mut crate::World) {
            S::update(state, world);
        }
    }
}
