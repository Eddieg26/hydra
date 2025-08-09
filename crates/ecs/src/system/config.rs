use super::{
    System, SystemCell, SystemCondition, SystemId, SystemInit, SystemMeta, SystemName, SystemRun,
    SystemState, SystemUpdate,
};
use crate::{AccessError, Frame, Resource, SystemArg, World, WorldAccess, WorldMode};
use std::{cell::UnsafeCell, collections::HashSet};

pub struct SystemConfig {
    id: SystemId,
    name: Option<SystemName>,
    exclusive: bool,
    send: bool,
    dependencies: HashSet<SystemId>,
    init: SystemInit,
    run: SystemRun,
    update: SystemUpdate,
    condition: SystemCondition,
}

impl SystemConfig {
    pub fn new(
        name: Option<impl Into<SystemName>>,
        exclusive: bool,
        send: bool,
        init: SystemInit,
        update: SystemUpdate,
        run: SystemRun,
        condition: SystemCondition,
    ) -> Self {
        Self {
            id: SystemId::new(),
            name: name.map(|name| name.into()),
            exclusive,
            send,
            dependencies: HashSet::new(),
            init,
            run,
            update,
            condition,
        }
    }

    pub fn id(&self) -> SystemId {
        self.id
    }

    pub fn name(&self) -> Option<&SystemName> {
        self.name.as_ref()
    }

    pub fn exclusive(&self) -> bool {
        self.exclusive
    }

    pub fn send(&self) -> bool {
        self.send
    }

    pub fn dependencies(&self) -> &HashSet<SystemId> {
        &self.dependencies
    }

    pub fn add_dependency(&mut self, id: SystemId) {
        self.dependencies.insert(id);
    }

    pub fn into_node(self, world: &mut World) -> SystemNode {
        let mut access = WorldAccess::new();
        let state = (self.init)(world, &mut access);

        if let Err(error) = access.validate() {
            match error {
                AccessError::Component(id) => {
                    let component = world.components().get_by_id(id).unwrap();
                    panic!(
                        "Invalid component access: {} for system: {:?}",
                        component.name(),
                        self.name
                    );
                }
                AccessError::Resource(id) => {
                    let resource = world.resources().get_meta(id).unwrap();
                    panic!(
                        "Invalid resource access: {} for system: {:?}",
                        resource.name(),
                        self.name
                    );
                }
            }
        }

        SystemNode {
            config: self,
            state,
            access,
        }
    }
}

pub struct SystemNode {
    pub config: SystemConfig,
    pub state: SystemState,
    pub access: WorldAccess,
}

impl SystemNode {
    pub fn has_dependency(&self, other: &SystemNode) -> bool {
        if self.config.dependencies.contains(&other.config.id) {
            return true;
        }

        self.access.conflicts(&other.access).is_err()
    }
}

pub enum SystemConfigs {
    Config(SystemConfig),
    Configs(Vec<SystemConfig>),
}

impl SystemConfigs {
    pub fn single(self) -> SystemConfig {
        match self {
            SystemConfigs::Config(config) => config,
            SystemConfigs::Configs(configs) => configs.into_iter().next().unwrap(),
        }
    }

    pub fn flatten(self) -> Vec<SystemConfig> {
        match self {
            SystemConfigs::Config(config) => vec![config],
            SystemConfigs::Configs(configs) => configs,
        }
    }
}

impl AsRef<SystemConfig> for SystemConfigs {
    fn as_ref(&self) -> &SystemConfig {
        match self {
            SystemConfigs::Config(config) => config,
            SystemConfigs::Configs(configs) => &configs[0],
        }
    }
}

impl AsMut<SystemConfig> for SystemConfigs {
    fn as_mut(&mut self) -> &mut SystemConfig {
        match self {
            SystemConfigs::Config(config) => config,
            SystemConfigs::Configs(configs) => &mut configs[0],
        }
    }
}

impl SystemConfigs {
    pub fn new(config: SystemConfig) -> Self {
        SystemConfigs::Config(config)
    }

    pub fn configs(configs: Vec<SystemConfig>) -> Self {
        SystemConfigs::Configs(configs)
    }

    pub fn config(&self) -> &SystemConfig {
        match self {
            SystemConfigs::Config(config) => config,
            SystemConfigs::Configs(configs) => &configs[0],
        }
    }
}

impl IntoSystemConfig<()> for SystemConfig {
    fn config(self) -> SystemConfig {
        self
    }
}

pub trait IntoSystemConfig<M>: Sized {
    fn config(self) -> SystemConfig;

    fn when<C: Condition>(self) -> SystemConfig {
        let mut config = self.config();
        config.condition = C::evaluate;
        config
    }
}

impl<M, I: IntoSystemConfig<M>> IntoSystemConfigs<M> for I {
    fn configs(self) -> SystemConfigs {
        SystemConfigs::Config(self.config())
    }

    fn before<Marker>(self, configs: impl IntoSystemConfigs<Marker>) -> SystemConfigs {
        self.configs().before(configs)
    }
}

pub trait IntoSystemConfigs<M> {
    fn configs(self) -> SystemConfigs;

    fn before<Marker>(self, configs: impl IntoSystemConfigs<Marker>) -> SystemConfigs;

    fn after<Marker>(self, configs: impl IntoSystemConfigs<Marker>) -> SystemConfigs
    where
        Self: Sized,
    {
        configs.before(self)
    }

    fn when<C: Condition>(self) -> SystemConfigs
    where
        Self: Sized,
    {
        match self.configs() {
            SystemConfigs::Config(mut config) => {
                config.condition = C::evaluate;
                SystemConfigs::Config(config)
            }
            SystemConfigs::Configs(mut configs) => {
                configs.iter_mut().for_each(|c| c.condition = C::evaluate);
                SystemConfigs::Configs(configs)
            }
        }
    }

    fn non_send(self) -> SystemConfigs
    where
        Self: Sized,
    {
        match self.configs() {
            SystemConfigs::Config(mut config) => {
                config.send = false;
                SystemConfigs::Config(config)
            }
            SystemConfigs::Configs(mut configs) => {
                configs.iter_mut().for_each(|c| c.send = false);
                SystemConfigs::Configs(configs)
            }
        }
    }
}

impl IntoSystemConfigs<()> for SystemConfigs {
    fn configs(self) -> SystemConfigs {
        self
    }

    fn before<Marker>(self, configs: impl IntoSystemConfigs<Marker>) -> SystemConfigs {
        let configs = configs.configs();

        match (self, configs) {
            (SystemConfigs::Config(before), SystemConfigs::Config(mut after)) => {
                after.dependencies.insert(before.id);
                Self::Configs(vec![before, after])
            }
            (SystemConfigs::Config(before), SystemConfigs::Configs(mut after)) => {
                after.iter_mut().for_each(|s| {
                    s.dependencies.insert(before.id);
                });
                after.insert(0, before);
                Self::Configs(after)
            }
            (SystemConfigs::Configs(mut before), SystemConfigs::Config(mut after)) => {
                after.dependencies.extend(before.iter().map(|s| s.id));
                before.push(after);
                Self::Configs(before)
            }
            (SystemConfigs::Configs(mut before), SystemConfigs::Configs(mut after)) => {
                after
                    .iter_mut()
                    .for_each(|s| s.dependencies.extend(before.iter().map(|s| s.id)));
                before.extend(after);
                Self::Configs(before)
            }
        }
    }
}

impl<F: Fn() + Send + Sync + 'static> IntoSystemConfig<()> for F {
    fn config(self) -> SystemConfig {
        SystemConfig {
            id: SystemId::new(),
            name: None,
            exclusive: false,
            send: true,
            dependencies: HashSet::new(),
            init: |_, _| Box::new(()),
            run: Box::new(move |_, _, _| {
                self();
            }),
            update: |_, _| {},
            condition: |_, _| true,
        }
    }
}

impl IntoSystemConfigs<()> for Vec<SystemConfig> {
    fn configs(self) -> SystemConfigs {
        SystemConfigs::Configs(self)
    }

    fn before<Marker>(mut self, configs: impl IntoSystemConfigs<Marker>) -> SystemConfigs {
        match configs.configs() {
            SystemConfigs::Config(config) => self.push(config),
            SystemConfigs::Configs(configs) => self.extend(configs),
        };

        SystemConfigs::Configs(self)
    }
}

impl From<SystemNode> for System {
    fn from(value: SystemNode) -> Self {
        let resources = value.access.current.resources.collect();
        let mut components = value.access.current.components.collect();
        for archetype in value.access.current.archetypes {
            components.union_with(archetype.access().get_read());
            components.union_with(archetype.access().get_write());
        }

        let meta = SystemMeta {
            id: value.config.id,
            name: value.config.name,
            components,
            resources,
            send: value.config.send,
            exclusive: value.config.exclusive,
            frame: Frame::ZERO,
        };

        System::new(
            meta,
            value.state,
            value.config.run,
            value.config.update,
            value.config.condition,
        )
    }
}

impl From<SystemNode> for SystemCell {
    fn from(node: SystemNode) -> Self {
        Self(UnsafeCell::new(node.into()))
    }
}

pub struct Not<T>(T);
pub struct Or<T>(T);
pub struct Modified<T>(std::marker::PhantomData<T>);
pub struct Added<T>(std::marker::PhantomData<T>);
pub struct Removed<T>(T);

impl<T> Removed<T> {
    pub fn into(self) -> T {
        self.0
    }
}

impl<T> From<T> for Removed<T> {
    fn from(value: T) -> Self {
        Self(value)
    }
}

impl<T> std::ops::Deref for Removed<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> std::ops::DerefMut for Removed<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<T> AsRef<T> for Removed<T> {
    fn as_ref(&self) -> &T {
        &self.0
    }
}

impl<T> AsMut<T> for Removed<T> {
    fn as_mut(&mut self) -> &mut T {
        &mut self.0
    }
}

unsafe impl<T: Resource + Send> SystemArg for Option<Removed<T>> {
    type Item<'world, 'state> = Self;

    type State = ();

    fn init(world: &mut World, access: &mut WorldAccess) -> Self::State {
        access.resources_mut().reads(world.register_resource::<T>());
    }

    unsafe fn get<'world, 'state>(
        _: &'state mut Self::State,
        mut world: crate::world::WorldCell<'world>,
        _: &'world SystemMeta,
    ) -> Self::Item<'world, 'state> {
        unsafe { world.get_mut().remove_resource::<T>().map(Removed) }
    }
}

pub trait Condition: Sized + 'static {
    fn evaluate(world: &World, system: &SystemMeta) -> bool;

    fn and(self, condition: impl Condition) -> impl Condition {
        (self, condition)
    }

    fn or(self, condition: impl Condition) -> impl Condition {
        Or((self, condition))
    }

    fn not(self) -> impl Condition {
        Not(self)
    }
}

pub struct Always<const VALUE: bool>(pub bool);

impl<const VALUE: bool> Condition for Always<VALUE> {
    fn evaluate(_: &World, _: &SystemMeta) -> bool {
        VALUE
    }
}

impl<T: Condition> Condition for Not<T> {
    fn evaluate(world: &World, system: &SystemMeta) -> bool {
        !T::evaluate(world, system)
    }
}

pub struct CurrentMode<M: WorldMode>(std::marker::PhantomData<M>);
impl<M: WorldMode> Condition for CurrentMode<M> {
    fn evaluate(world: &World, _: &SystemMeta) -> bool {
        let id = world.modes.id::<M>();

        world.modes.current() == id
    }
}

pub struct Entered<M: WorldMode>(std::marker::PhantomData<M>);
impl<M: WorldMode> Condition for Entered<M> {
    fn evaluate(world: &World, system: &SystemMeta) -> bool {
        world.modes.id::<M>().is_some_and(|id| {
            let entered = world.modes[id].frame();
            world.modes.current() == Some(id) && entered.is_newer(world.frame, system.frame)
        })
    }
}

pub struct Exited<M: WorldMode>(std::marker::PhantomData<M>);
impl<M: WorldMode> Condition for Exited<M> {
    fn evaluate(world: &World, system: &SystemMeta) -> bool {
        world.modes.id::<M>().is_some_and(|id| {
            let exited = world.modes[id].frame();
            world.modes.current() != Some(id) && exited.is_newer(world.frame, system.frame)
        })
    }
}

pub struct Exists<R: Resource>(std::marker::PhantomData<R>);
impl<R: Resource> Condition for Exists<R> {
    fn evaluate(world: &World, _: &SystemMeta) -> bool {
        let id = match world.resources.get_id::<R>() {
            Some(id) => id,
            None => return false,
        };

        world
            .resources
            .get_meta(id)
            .is_some_and(|meta| meta.exists() && meta.has_access())
    }
}

impl<R: Resource> Condition for Modified<R> {
    fn evaluate(world: &World, system: &SystemMeta) -> bool {
        let id = match world.resources.get_id::<R>() {
            Some(id) => id,
            None => return false,
        };

        world.resources.get_meta(id).is_some_and(|meta| {
            meta.exists() && meta.modified().is_newer(world.frame, system.frame)
        })
    }
}

impl<R: Resource> Condition for Added<R> {
    fn evaluate(world: &World, system: &SystemMeta) -> bool {
        let id = match world.resources.get_id::<R>() {
            Some(id) => id,
            None => return false,
        };

        world
            .resources
            .get_meta(id)
            .is_some_and(|meta| meta.exists() && meta.added().is_newer(world.frame, system.frame))
    }
}

impl<R: Resource> Condition for Removed<R> {
    fn evaluate(world: &World, system: &SystemMeta) -> bool {
        let id = match world.resources.get_id::<R>() {
            Some(id) => id,
            None => return false,
        };

        world.resources.get_meta(id).is_some_and(|meta| {
            !meta.exists() && meta.removed().is_newer(world.frame, system.frame)
        })
    }
}

macro_rules! impl_tuple_condition {
    ($($name:ident),*) => {
        #[allow(non_snake_case)]
        impl<$($name: Condition),*> Condition for ($($name),*) {
            fn evaluate(world: &World, meta: &SystemMeta) -> bool {
                let mut result = true;
                $(
                    result = result && $name::evaluate(world, meta);
                )*
                result
            }
        }

        #[allow(non_snake_case)]
        impl<$($name: Condition),*> Condition for Or<($($name),*)> {
            fn evaluate( world: &World, meta: &SystemMeta) -> bool {
                let mut result = false;
                $(
                    result = result || $name::evaluate(world, meta);
                )*
                result
            }
        }
    };
}

variadics::variable_impl!(impl_tuple_condition, P, 2, 16);

#[allow(unused_imports, dead_code)]
mod tests {
    use super::{Condition, CurrentMode, Exists};
    use crate::{
        IntoSystemConfig, Resource, System, SystemConfigs, SystemMeta, World, WorldMode,
        system::{Added, Entered, Exited, Modified, Not, Or, Removed},
        world,
    };

    #[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Value(u32);
    impl Resource for Value {}

    pub struct Ghost;
    impl Resource for Ghost {}

    pub struct TestMode;
    impl WorldMode for TestMode {}

    #[test]
    fn test_resource_exists() {
        let mut world = World::new();
        world.add_resource(Value(0));

        let system = SystemMeta::default();
        assert!(Exists::<Value>::evaluate(&world, &system));
        assert!(Not::<Exists<Ghost>>::evaluate(&world, &system));
    }

    #[test]
    fn test_resource_added() {
        let mut world = World::new();
        world.add_resource(Value(0));

        let system = SystemMeta::default();
        assert!(Added::<Value>::evaluate(&world, &system));
        assert!(Not::<Added<Ghost>>::evaluate(&world, &system));
    }

    #[test]
    fn test_resource_modified() {
        let mut world = World::new();
        world.add_resource(Value(0));
        world.add_resource(Ghost);

        world.resource_mut::<Value>();

        let system = SystemMeta::default();
        assert!(Modified::<Value>::evaluate(&world, &system));
        assert!(Not::<Modified<Ghost>>::evaluate(&world, &system));
    }

    #[test]
    fn test_resource_removed() {
        let mut world = World::new();
        world.add_resource(Value(0));
        world.remove_resource::<Value>().unwrap();

        let system = SystemMeta::default();
        assert!(Removed::<Value>::evaluate(&world, &system));
    }

    #[test]
    fn test_or_condition() {
        let mut world = World::new();
        world.add_resource(Ghost);

        let system = SystemMeta::default();
        assert!(Or::<(Exists<Value>, Exists<Ghost>)>::evaluate(
            &world, &system
        ));
    }

    #[test]
    fn test_run_conditional_system() {
        let mut world = World::new();
        world.add_resource(Value(0));

        let systems = vec![
            (|value: &mut Value| {
                value.0 = 1;
            })
            .when::<Exists<Value>>(),
            (|_: &Ghost| {}).when::<Exists<Ghost>>(),
        ]
        .drain(..)
        .map(|config| System::from(config.into_node(&mut world)))
        .collect::<Vec<_>>();

        for mut system in systems {
            system.run(unsafe { world.cell() });
        }

        assert_eq!(world.resource::<Value>().0, 1);
    }

    #[test]
    fn test_current_mode() {
        let mut world = World::new();
        world.add_mode::<TestMode>();

        world.enter::<TestMode>();

        let system = SystemMeta::default();
        assert!(CurrentMode::<TestMode>::evaluate(&world, &system))
    }

    #[test]
    fn test_enter_mode() {
        let mut world = World::new();
        world.add_mode::<TestMode>();

        world.enter::<TestMode>();

        let system = SystemMeta::default();
        assert!(Entered::<TestMode>::evaluate(&world, &system))
    }

    #[test]
    fn test_exit_mode() {
        let mut world = World::new();
        world.add_mode::<TestMode>();

        world.enter::<TestMode>();
        world.exit();

        let system = SystemMeta::default();
        assert!(Exited::<TestMode>::evaluate(&world, &system))
    }
}
