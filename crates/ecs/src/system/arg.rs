use super::{IntoSystemConfig, SystemConfig, SystemId, SystemInit, SystemMeta};
use crate::{
    Cloned, CommandBuffer, Commands,
    world::{Entities, NonSend, NonSendMut, Resource, ResourceId, World, WorldCell},
};
use std::any::Any;

#[allow(unused_variables)]
pub unsafe trait SystemArg: Sized {
    type Item<'world, 'state>: SystemArg<State = Self::State>;

    type State: Send + Sync + 'static;

    fn init(system: &mut SystemInit) -> Self::State;

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

    fn apply(state: &mut Self::State, world: &mut World) {}
}

pub type ArgItem<'world, 'state, A> = <A as SystemArg>::Item<'world, 'state>;

unsafe impl SystemArg for () {
    type Item<'world, 'state> = ();

    type State = ();

    fn init(_: &mut SystemInit) -> Self::State {
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

    fn init(_: &mut SystemInit) -> Self::State {
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
        true
    }
}

unsafe impl SystemArg for &Entities {
    type Item<'world, 'state> = &'world Entities;

    type State = ();

    fn init(_: &mut SystemInit) -> Self::State {
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

    fn init(_: &mut SystemInit) -> Self::State {
        CommandBuffer::new()
    }

    fn apply(state: &mut Self::State, world: &mut World) {
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

    fn init(system: &mut SystemInit) -> Self::State {
        system.read_resource::<R>()
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

    fn init(system: &mut SystemInit) -> Self::State {
        system.write_resource::<R>()
    }

    unsafe fn get<'world, 'state>(
        state: &'state mut Self::State,
        mut world: WorldCell<'world>,
        _system: &SystemMeta,
    ) -> Self::Item<'world, 'state> {
        unsafe { world.get_mut() }
            .resources_mut()
            .get_mut::<R>(*state)
            .unwrap()
    }
}

unsafe impl<R: Resource> SystemArg for NonSend<'_, R> {
    type Item<'world, 'state> = NonSend<'world, R>;

    type State = ResourceId;

    fn init(system: &mut SystemInit) -> Self::State {
        system.read_non_send_resource::<R>()
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

    fn init(system: &mut SystemInit) -> Self::State {
        system.write_non_send_resource::<R>()
    }

    unsafe fn get<'world, 'state>(
        state: &'state mut Self::State,
        mut world: WorldCell<'world>,
        _system: &SystemMeta,
    ) -> Self::Item<'world, 'state> {
        let resource = unsafe { world.get_mut() }
            .resources_mut()
            .get_mut::<R>(*state)
            .unwrap();

        NonSendMut::new(resource)
    }

    fn send() -> bool {
        false
    }
}

unsafe impl<R: Resource + Send + Clone> SystemArg for Cloned<R> {
    type Item<'world, 'state> = Cloned<R>;

    type State = ResourceId;

    fn init(system: &mut SystemInit) -> Self::State {
        system.world.register_resource::<R>()
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

    fn init(system: &mut SystemInit) -> Self::State {
        A::init(system)
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

macro_rules! impl_into_system_configs {
    ($($arg:ident),*) => {
    #[allow(non_snake_case)]
    impl<F, $($arg: SystemArg),*> IntoSystemConfig<(F, $($arg),*)> for F
        where
            for<'world, 'state> F: Fn($($arg),*) + Fn($(ArgItem<'world,'state, $arg>),*) + Send + Sync + 'static,
        {

            type In = A;

            fn config(self) -> SystemConfig {
                let name = std::any::type_name::<F>();

                let init = |system: &mut super::SystemInit| {
                    let ($($arg,)*) = ($($arg::init(system),)*);
                    let state = ($($arg,)*);
                    Box::new(state) as Box<dyn Any + Send + Sync>
                };

                let execute = move |state: &mut Box<dyn Any + Send + Sync>, world: WorldCell, system: &SystemMeta| {
                    let ($($arg,)*) = state.downcast_mut::<($($arg::State,)*)>().unwrap();
                    let ($($arg,)*) = unsafe {($($arg::get($arg, world, system),)*)};

                    self($($arg,)*);
                };

                let apply = move |state: &mut Box<dyn Any + Send + Sync>, world: &mut World| {
                    let ($($arg,)*) = state.downcast_mut::<($($arg::State,)*)>().unwrap();
                    $($arg::apply($arg, world);)*
                };

                let send = ($($arg::send() &&)* true);
                let exclusive = ($($arg::exclusive() ||)* false);

                SystemConfig {
                    id: SystemId::new(),
                    name: Some(name.into()),
                    exclusive,
                    send,
                    dependencies: std::collections::HashSet::new(),
                    init,
                    run: Box::new(execute),
                    apply: Box::new(apply),
                }
            }
        }

        #[allow(non_snake_case)]
        unsafe impl<$($arg: SystemArg),*> SystemArg for ($($arg,)*) {
            type Item<'world, 'state> = ($($arg::Item<'world, 'state>,)*);
            type State = ($($arg::State,)*);

            fn init(system: &mut SystemInit) -> Self::State {
                let ($($arg,)*) = ($($arg::init(system),)*);
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

            fn apply(state: &mut Self::State, world: &mut World) {
                let ($($arg,)*) = state;
                $($arg::apply($arg, world);)*
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
