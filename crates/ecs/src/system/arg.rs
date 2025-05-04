use super::{IntoSystemConfig, SystemConfig, SystemMeta};
use crate::{
    Cloned, CommandBuffer, Commands, Event, EventReader, EventStorage, EventWriter, Events,
    WorldAccess,
    world::{Entities, NonSend, NonSendMut, Resource, ResourceId, World, WorldCell},
};
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
        system: &SystemMeta,
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
        _system: &SystemMeta,
    ) -> Self::Item<'world, 'state> {
        ()
    }
}

unsafe impl SystemArg for &World {
    type Item<'world, 'state> = &'world World;

    type State = ();

    fn init(_: &mut World, _: &mut WorldAccess) -> Self::State {
        ()
    }

    unsafe fn get<'world, 'state>(
        _state: &'state mut Self::State,
        world: WorldCell<'world>,
        _system: &SystemMeta,
    ) -> Self::Item<'world, 'state> {
        unsafe { world.get() }
    }

    fn exclusive() -> bool {
        false
    }
}

unsafe impl SystemArg for &Entities {
    type Item<'world, 'state> = &'world Entities;

    type State = ();

    fn init(_: &mut World, _: &mut WorldAccess) -> Self::State {
        ()
    }

    unsafe fn get<'world, 'state>(
        _state: &'state mut Self::State,
        world: WorldCell<'world>,
        _system: &SystemMeta,
    ) -> Self::Item<'world, 'state> {
        unsafe { world.get().entities() }
    }
}

unsafe impl SystemArg for Commands<'_, '_> {
    type Item<'world, 'state> = Commands<'world, 'state>;

    type State = CommandBuffer;

    fn init(_: &mut World, _: &mut WorldAccess) -> Self::State {
        CommandBuffer::new()
    }

    fn update(state: &mut Self::State, world: &mut World) {
        CommandBuffer::execute(state, world);
    }

    unsafe fn get<'world, 'state>(
        state: &'state mut Self::State,
        _: super::WorldCell<'world>,
        _: &crate::system::SystemMeta,
    ) -> Self::Item<'world, 'state> {
        Commands::new(state)
    }
}

unsafe impl<R: Resource + Send> SystemArg for &R {
    type Item<'world, 'state> = &'world R;

    type State = ResourceId;

    fn init(world: &mut World, access: &mut WorldAccess) -> Self::State {
        let id = world.register_resource::<R>();
        access.resources_mut().read(id);
        id
    }

    unsafe fn get<'world, 'state>(
        state: &'state mut Self::State,
        world: WorldCell<'world>,
        _system: &SystemMeta,
    ) -> Self::Item<'world, 'state> {
        unsafe { world.get() }.resources().get::<R>(*state).unwrap()
    }
}

unsafe impl<R: Resource + Send> SystemArg for &mut R {
    type Item<'world, 'state> = &'world mut R;

    type State = ResourceId;

    fn init(world: &mut World, access: &mut WorldAccess) -> Self::State {
        let id = world.register_resource::<R>();
        access.resources_mut().write(id);
        id
    }

    unsafe fn get<'world, 'state>(
        state: &'state mut Self::State,
        mut world: WorldCell<'world>,
        _system: &SystemMeta,
    ) -> Self::Item<'world, 'state> {
        let world = unsafe { world.get_mut() };
        world.resources.get_mut::<R>(*state, world.frame).unwrap()
    }
}

unsafe impl<R: Resource> SystemArg for NonSend<'_, R> {
    type Item<'world, 'state> = NonSend<'world, R>;

    type State = ResourceId;

    fn init(world: &mut World, access: &mut WorldAccess) -> Self::State {
        let id = world.register_non_send_resource::<R>();
        access.resources_mut().read(id);
        id
    }

    unsafe fn get<'world, 'state>(
        state: &'state mut Self::State,
        world: WorldCell<'world>,
        _system: &SystemMeta,
    ) -> Self::Item<'world, 'state> {
        let resource = unsafe { world.get() }.resources().get::<R>(*state).unwrap();

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

    unsafe fn get<'world, 'state>(
        state: &'state mut Self::State,
        mut world: WorldCell<'world>,
        _system: &SystemMeta,
    ) -> Self::Item<'world, 'state> {
        let world = unsafe { world.get_mut() };
        let resource = world.resources.get_mut::<R>(*state, world.frame).unwrap();

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

    unsafe fn get<'world, 'state>(
        state: &'state mut Self::State,
        world: WorldCell<'world>,
        _: &SystemMeta,
    ) -> Self::Item<'world, 'state> {
        let resource = unsafe { world.get() }
            .resources()
            .get::<R>(*state)
            .cloned()
            .unwrap();

        Cloned::new(resource)
    }
}

unsafe impl<A: SystemArg> SystemArg for Option<A> {
    type Item<'world, 'state> = Option<A::Item<'world, 'state>>;

    type State = A::State;

    fn init(world: &mut World, access: &mut WorldAccess) -> Self::State {
        A::init(world, access)
    }

    unsafe fn validate(state: &Self::State, world: WorldCell, system: &SystemMeta) -> bool {
        unsafe { A::validate(state, world, system) }
    }

    unsafe fn get<'world, 'state>(
        state: &'state mut Self::State,
        world: WorldCell<'world>,
        system: &SystemMeta,
    ) -> Self::Item<'world, 'state> {
        unsafe {
            if A::validate(state, world, system) {
                Some(A::get(state, world, system))
            } else {
                None
            }
        }
    }

    fn send() -> bool {
        A::send()
    }
}

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
        _: &crate::system::SystemMeta,
    ) -> Self::Item<'world, 'state> {
        let events = unsafe { world.get().resource::<Events<E>>() };
        EventReader::new(events)
    }
}

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
        _: &crate::system::SystemMeta,
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

macro_rules! impl_into_system_configs {
    ($($arg:ident),*) => {
    #[allow(non_snake_case)]
    impl<O, F, $($arg: SystemArg),*> IntoSystemConfig<(F, $($arg),*)> for F
        where
            for<'world, 'state> F: Fn($($arg),*) + Fn($(ArgItem<'world,'state, $arg>),*) -> O + Send + Sync + 'static,
        {
            fn config(self) -> SystemConfig {
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

                SystemConfig::new(Some(name), exclusive, send, init, update, Box::new(run), |_, _| true)
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

            unsafe fn get<'world, 'state>(state: &'state mut Self::State, world: WorldCell<'world>, system: &SystemMeta,) -> Self::Item<'world, 'state> {
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

impl_into_system_configs!(A);
impl_into_system_configs!(A, B);
impl_into_system_configs!(A, B, C);
impl_into_system_configs!(A, B, C, D);
impl_into_system_configs!(A, B, C, D, E);
impl_into_system_configs!(A, B, C, D, E, F2);
impl_into_system_configs!(A, B, C, D, E, F2, G);
impl_into_system_configs!(A, B, C, D, E, F2, G, H);
impl_into_system_configs!(A, B, C, D, E, F2, G, H, I);
impl_into_system_configs!(A, B, C, D, E, F2, G, H, I, J);
impl_into_system_configs!(A, B, C, D, E, F2, G, H, I, J, K);
impl_into_system_configs!(A, B, C, D, E, F2, G, H, I, J, K, L);
impl_into_system_configs!(A, B, C, D, E, F2, G, H, I, J, K, L, M);
impl_into_system_configs!(A, B, C, D, E, F2, G, H, I, J, K, L, M, N);
impl_into_system_configs!(A, B, C, D, E, F2, G, H, I, J, K, L, M, N, O2);
impl_into_system_configs!(A, B, C, D, E, F2, G, H, I, J, K, L, M, N, O2, P);
impl_into_system_configs!(A, B, C, D, E, F2, G, H, I, J, K, L, M, N, O2, P, Q);

#[allow(type_alias_bounds)]
pub mod unlifetime {
    use super::{ArgItem, SystemArg};
    use crate::{BaseFilter, BaseQuery, NonSend, NonSendMut, Query, Resource};

    pub type Read<T> = &'static T;
    pub type Write<T> = &'static mut T;
    pub type NonSendRes<R: Resource> = NonSend<'static, R>;
    pub type NonSendResMut<R: Resource> = NonSendMut<'static, R>;
    pub type SQuery<Q: BaseQuery, F: BaseFilter> = Query<'static, 'static, Q, F>;

    pub struct StaticArg<'w, 's, S: SystemArg>(ArgItem<'w, 's, S>);
    impl<'w, 's, S: SystemArg> StaticArg<'w, 's, S> {
        pub fn get(&self) -> &ArgItem<'w, 's, S> {
            &self.0
        }

        pub fn get_mut(&mut self) -> &mut ArgItem<'w, 's, S> {
            &mut self.0
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
            world: crate::WorldCell<'world>,
            system: &crate::SystemMeta,
        ) -> Self::Item<'world, 'state> {
            unsafe { StaticArg(S::get(state, world, system)) }
        }
    }
}
