use std::any::TypeId;

use crate::{Entity, Resource, SystemArg, World, WorldAccess, WorldMode, world::WorldCell};

pub mod entity;

pub trait Command: Sized + Send + 'static {
    fn execute(self, world: &mut World);
}

#[repr(C, packed)]
struct Packed<C: Command> {
    execute: ExecuteCommand,
    command: C,
}

impl<C: Command> Packed<C> {
    pub fn new(command: C) -> Self {
        Self {
            execute: |bytes, world| {
                let command = unsafe { std::ptr::read::<C>(bytes.as_ptr() as *const C) };
                command.execute(world);

                std::mem::size_of::<C>()
            },
            command,
        }
    }
}

pub struct RawCommand(Vec<u8>);

impl RawCommand {
    pub fn new<C: Command>(command: C) -> Self {
        let mut data = vec![0; std::mem::size_of::<Packed::<C>>()];

        unsafe {
            data.as_mut_ptr()
                .cast::<Packed<C>>()
                .write_unaligned(Packed::new(command));
        };

        Self(data)
    }
}

impl<C: Command> From<C> for RawCommand {
    fn from(command: C) -> Self {
        Self::new(command)
    }
}

impl<F: FnOnce(&mut World) + Send + 'static> Command for F {
    fn execute(self, world: &mut World) {
        self(world)
    }
}

impl<F: FnOnce(Entity, &mut World) + Send + 'static> EntityCommand for F {
    fn execute(self, entity: Entity, world: &mut World) {
        self(entity, world)
    }
}

pub trait EntityCommand: Send + 'static {
    fn execute(self, entity: Entity, world: &mut World);

    fn with_entity(self, entity: Entity) -> impl Command
    where
        Self: Sized,
    {
        move |world: &mut World| self.execute(entity, world)
    }
}

pub type ExecuteCommand = fn(&[u8], &mut World) -> usize;

pub struct CommandBuffer {
    buffer: Vec<u8>,
}

impl CommandBuffer {
    pub fn new() -> Self {
        Self { buffer: vec![] }
    }

    pub fn add<C: Command>(&mut self, command: C) {
        unsafe {
            let offset = self.buffer.len();
            self.buffer.reserve(std::mem::size_of::<Packed<C>>());

            let ptr = self.buffer.as_mut_ptr().add(offset);

            ptr.cast::<Packed<C>>()
                .write_unaligned(Packed::new(command));

            self.buffer
                .set_len(offset + std::mem::size_of::<Packed<C>>());
        };
    }

    pub fn extend(&mut self, commands: CommandBuffer) {
        self.buffer.extend(commands.buffer);
    }

    pub unsafe fn add_raw(&mut self, command: RawCommand) {
        self.buffer.extend(command.0);
    }

    pub fn execute(&mut self, world: &mut World) {
        let mut start = 0;

        while start < self.buffer.len() {
            let executor = unsafe {
                self.buffer[start..start + std::mem::size_of::<ExecuteCommand>()]
                    .as_ptr()
                    .cast::<ExecuteCommand>()
                    .as_ref()
                    .unwrap_unchecked()
            };

            start += std::mem::size_of::<ExecuteCommand>();

            let command = &self.buffer[start..];
            start += executor(command, world);
        }

        self.buffer.clear();
    }
}

pub struct Commands<'world, 'state> {
    commands: &'state mut CommandBuffer,
    _marker: std::marker::PhantomData<&'world ()>,
}

impl<'world, 'state> Commands<'world, 'state> {
    pub fn new(commands: &'state mut CommandBuffer) -> Self {
        Commands {
            commands,
            _marker: std::marker::PhantomData,
        }
    }

    pub fn add<C: Command>(&mut self, command: C) {
        self.commands.add(command);
    }

    pub fn add_buffer(&mut self, commands: CommandBuffer) {
        self.commands.extend(commands);
    }

    pub fn entity(&mut self, entity: Entity) -> EntityCommands {
        EntityCommands {
            entity,
            buffer: self.commands,
        }
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
        _: WorldCell<'world>,
        _: &crate::system::SystemMeta,
    ) -> Self::Item<'world, 'state> {
        Commands::new(state)
    }
}

pub struct AddResource<R: Resource + Send>(R);
impl<R: Resource + Send> From<R> for AddResource<R> {
    fn from(value: R) -> Self {
        Self(value)
    }
}

impl<R: Resource + Send> Command for AddResource<R> {
    fn execute(self, world: &mut World) {
        world.add_resource(self.0);
    }
}

pub struct RemoveResource(TypeId);
impl RemoveResource {
    pub fn new<R: Resource>() -> Self {
        Self(TypeId::of::<R>())
    }
}

impl Command for RemoveResource {
    fn execute(self, world: &mut World) {
        if let Some(id) = world.resources.get_id_dynamic(&self.0) {
            world.resources.remove_by_id(id, world.frame);
        }
    }
}

pub struct EnterMode<M: WorldMode>(std::marker::PhantomData<M>);
impl<M: WorldMode> Default for EnterMode<M> {
    fn default() -> Self {
        Self(Default::default())
    }
}

impl<M: WorldMode> Command for EnterMode<M> {
    fn execute(self, world: &mut World) {
        world.enter::<M>();
    }
}

pub struct ExitMode;
impl Command for ExitMode {
    fn execute(self, world: &mut World) {
        world.exit();
    }
}

pub struct EntityCommands<'a> {
    pub(crate) entity: Entity,
    pub(crate) buffer: &'a mut CommandBuffer,
}

impl<'a> EntityCommands<'a> {
    pub fn new(entity: Entity, buffer: &'a mut CommandBuffer) -> Self {
        Self { entity, buffer }
    }

    pub fn add(&mut self, command: impl EntityCommand) {
        self.buffer.add(command.with_entity(self.entity));
    }

    pub fn entity(&self) -> Entity {
        self.entity
    }

    pub fn get(&'a mut self, entity: Entity) -> Self {
        Self {
            entity,
            buffer: self.buffer,
        }
    }

    pub fn finish(self) -> Entity {
        self.entity
    }
}

#[allow(unused_imports, dead_code)]
mod tests {
    use super::{Command, CommandBuffer};
    use crate::{Resource, World};

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Value(usize);
    impl Resource for Value {}

    pub struct Test;
    impl Command for Test {
        fn execute(self, world: &mut crate::World) {
            world.resource_mut::<Value>().0 += 1;
        }
    }

    #[test]
    fn command_buffer() {
        let mut world = World::new();
        let mut buffer = CommandBuffer::new();
        let count = (0..3).map(|_| buffer.add(Test)).collect::<Vec<_>>().len();

        world.add_resource(Value(0));
        buffer.execute(&mut world);

        assert_eq!(world.resource::<Value>().0, count);
    }
}
