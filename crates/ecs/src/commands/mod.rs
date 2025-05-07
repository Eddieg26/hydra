use crate::{Entity, World, WorldMode};

pub mod entity;

pub trait Command: Sized + Send + 'static {
    fn execute(self, world: &mut World);
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
        #[repr(C, packed)]
        struct RawCommand<C: Command> {
            execute: ExecuteCommand,
            command: C,
        }

        impl<C: Command> RawCommand<C> {
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

        unsafe {
            let offset = self.buffer.len();
            self.buffer.reserve(std::mem::size_of::<RawCommand<C>>());

            let ptr = self.buffer.as_mut_ptr().add(offset);

            ptr.cast::<RawCommand<C>>()
                .write_unaligned(RawCommand::new(command));

            self.buffer
                .set_len(offset + std::mem::size_of::<RawCommand<C>>());
        };
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

    pub fn entity(&mut self, entity: Entity) -> EntityCommands {
        EntityCommands {
            entity,
            buffer: self.commands,
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
