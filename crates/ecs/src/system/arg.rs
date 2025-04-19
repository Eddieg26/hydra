use super::{IntoSystemConfigs, SystemConfig, SystemConfigs, SystemId, SystemMeta};
use crate::{
    system::{Access, SystemAccess},
    world::{Entities, NonSend, NonSendMut, Resource, ResourceId, World, WorldCell},
};
use std::any::Any;

#[allow(unused_variables)]
pub unsafe trait SystemArg: Sized {
    type Item<'world, 'state>: SystemArg<State = Self::State>;

    type State: Send + Sync + 'static;

    fn init(world: &mut World) -> Self::State;

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

    fn access(state: &Self::State) -> Vec<SystemAccess> {
        vec![]
    }
}

pub type ArgItem<'world, 'state, A> = <A as SystemArg>::Item<'world, 'state>;

unsafe impl SystemArg for () {
    type Item<'world, 'state> = ();

    type State = ();

    fn init(_: &mut World) -> Self::State {
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

    fn init(_: &mut World) -> Self::State {
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

    fn init(_: &mut World) -> Self::State {
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

unsafe impl<R: Resource + Send> SystemArg for &R {
    type Item<'world, 'state> = &'world R;

    type State = ResourceId;

    fn init(world: &mut World) -> Self::State {
        world.register_resource::<R>()
    }

    unsafe fn get<'world, 'state>(
        _state: &'state mut Self::State,
        world: WorldCell<'world>,
        _system: &SystemMeta,
    ) -> Self::Item<'world, 'state> {
        unsafe { world.get().resource() }
    }

    fn access(state: &Self::State) -> Vec<SystemAccess> {
        vec![SystemAccess::resource(*state, Access::Read)]
    }
}

unsafe impl<R: Resource + Send> SystemArg for &mut R {
    type Item<'world, 'state> = &'world mut R;

    type State = ResourceId;

    fn init(world: &mut World) -> Self::State {
        world.register_resource::<R>()
    }

    unsafe fn get<'world, 'state>(
        _state: &'state mut Self::State,
        mut world: WorldCell<'world>,
        _system: &SystemMeta,
    ) -> Self::Item<'world, 'state> {
        unsafe { world.get_mut().resource_mut() }
    }

    fn access(state: &Self::State) -> Vec<SystemAccess> {
        vec![SystemAccess::resource(*state, Access::Write)]
    }
}

unsafe impl<R: Resource> SystemArg for NonSend<'_, R> {
    type Item<'world, 'state> = NonSend<'world, R>;

    type State = ResourceId;

    fn init(world: &mut World) -> Self::State {
        world.register_non_send_resource::<R>()
    }

    unsafe fn get<'world, 'state>(
        _state: &'state mut Self::State,
        world: WorldCell<'world>,
        _system: &SystemMeta,
    ) -> Self::Item<'world, 'state> {
        let resource = unsafe { world.get().non_send_resource::<R>() };

        NonSend::new(resource)
    }

    fn send() -> bool {
        false
    }

    fn access(state: &Self::State) -> Vec<SystemAccess> {
        vec![SystemAccess::resource(*state, Access::Read)]
    }
}

unsafe impl<R: Resource> SystemArg for NonSendMut<'_, R> {
    type Item<'world, 'state> = NonSendMut<'world, R>;

    type State = ResourceId;

    fn init(world: &mut World) -> Self::State {
        world.register_non_send_resource::<R>()
    }

    unsafe fn get<'world, 'state>(
        _state: &'state mut Self::State,
        mut world: WorldCell<'world>,
        _system: &SystemMeta,
    ) -> Self::Item<'world, 'state> {
        let resource = unsafe { world.get_mut().non_send_resource_mut::<R>() };

        NonSendMut::new(resource)
    }

    fn send() -> bool {
        false
    }

    fn access(state: &Self::State) -> Vec<SystemAccess> {
        vec![SystemAccess::resource(*state, Access::Write)]
    }
}

unsafe impl<A: SystemArg> SystemArg for Option<A> {
    type Item<'world, 'state> = Option<A::Item<'world, 'state>>;

    type State = A::State;

    fn init(world: &mut World) -> Self::State {
        A::init(world)
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

    fn access(state: &Self::State) -> Vec<SystemAccess> {
        A::access(state)
    }
}

macro_rules! impl_into_system_configs {
    ($($arg:ident),*) => {
    #[allow(non_snake_case)]
    impl<F, $($arg: SystemArg),*> IntoSystemConfigs<(F, $($arg),*)> for F
        where
            for<'world, 'state> F: Fn($($arg),*) + Fn($(ArgItem<'world,'state, $arg>),*) + Send + Sync + 'static,
        {

            fn configs(self) -> SystemConfigs {
                let name = std::any::type_name::<F>();

                let init = |world: &mut World| {
                    let ($($arg,)*) = ($($arg::init(world),)*);
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

                let access = |state: &Box<dyn Any + Send + Sync>| {
                    let ($($arg,)*) = state.downcast_ref::<($($arg::State,)*)>().unwrap();
                    let mut access = Vec::new();
                    $(access.extend($arg::access($arg));)*
                    access
                };

                let send = ($($arg::send() &&)* true);
                let exclusive = ($($arg::exclusive() ||)* false);

                SystemConfigs::Config(SystemConfig {
                    id: SystemId::new(),
                    name: Some(name.into()),
                    exclusive,
                    send,
                    dependencies: std::collections::HashSet::new(),
                    init,
                    run: Box::new(execute),
                    apply: Box::new(apply),
                    access
                })
            }

            fn before<Marker>(self, configs: impl IntoSystemConfigs<Marker>) -> SystemConfigs {
                let before = self.configs().single();
                let after_configs = configs.configs();

                match after_configs {
                    SystemConfigs::Config(mut config) => {
                        config.dependencies.insert(before.id);
                        SystemConfigs::Configs(vec![before, config])
                    }
                    SystemConfigs::Configs(mut configs) => {
                        configs.iter_mut().for_each(|config| {
                            config.dependencies.insert(before.id);
                        });

                        configs.insert(0, before);
                        SystemConfigs::Configs(configs)
                    }
                }
            }

            fn after<Marker>(self, configs: impl IntoSystemConfigs<Marker>) -> SystemConfigs {
                let configs = configs.configs();
                configs.before(self)
            }
        }

        #[allow(non_snake_case)]
        unsafe impl<$($arg: SystemArg),*> SystemArg for ($($arg,)*) {
            type Item<'world, 'state> = ($($arg::Item<'world, 'state>,)*);
            type State = ($($arg::State,)*);

            fn init(world: &mut World) -> Self::State {
                let ($($arg,)*) = ($($arg::init(world),)*);
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

            fn access(state: &Self::State) -> Vec<SystemAccess> {
                let ($($arg,)*) = state;
                let mut access = Vec::new();
                $(access.extend($arg::access($arg));)*
                access
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
