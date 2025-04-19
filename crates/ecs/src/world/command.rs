use super::{Component, Entity, Row, World};
use crate::system::arg::SystemArg;

pub trait Command: Sized + Send + Sync + 'static {
    fn execute(self, world: &mut World);
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
}

unsafe impl SystemArg for Commands<'_, '_> {
    type Item<'world, 'state> = Commands<'world, 'state>;

    type State = CommandBuffer;

    fn init(_: &mut World) -> Self::State {
        CommandBuffer::new()
    }

    fn apply(state: &mut Self::State, world: &mut World) {
        state.execute(world);
    }

    unsafe fn get<'world, 'state>(
        state: &'state mut Self::State,
        _: super::WorldCell<'world>,
        _: &crate::system::SystemMeta,
    ) -> Self::Item<'world, 'state> {
        Commands::new(state)
    }
}

pub struct Spawner<'world, 'state> {
    world: &'world mut World,
    entities: &'state mut Vec<(Entity, Row)>,
    _marker: std::marker::PhantomData<&'state ()>,
}

impl<'world, 'state> Spawner<'world, 'state> {
    pub fn new(world: &'world mut World, entities: &'state mut Vec<(Entity, Row)>) -> Self {
        Spawner {
            world,
            entities,
            _marker: std::marker::PhantomData,
        }
    }

    pub fn spawn(&mut self) -> Spawned<'world, 'state, '_> {
        let id = self.world.spawn();
        Spawned {
            id,
            components: Row::new(),
            spawner: self,
        }
    }
}

unsafe impl SystemArg for Spawner<'_, '_> {
    type Item<'world, 'state> = Spawner<'world, 'state>;

    type State = Vec<(Entity, Row)>;

    fn init(_: &mut World) -> Self::State {
        vec![]
    }

    unsafe fn get<'world, 'state>(
        state: &'state mut Self::State,
        mut world: super::WorldCell<'world>,
        _: &crate::system::SystemMeta,
    ) -> Self::Item<'world, 'state> {
        unsafe { Spawner::new(world.get_mut(), state) }
    }

    fn exclusive() -> bool {
        true
    }

    fn apply(state: &mut Self::State, world: &mut World) {
        for (entity, components) in state.drain(..) {
            world.add_components(entity, components);
        }
    }
}

pub struct Spawned<'world, 'state, 'spawner> {
    id: Entity,
    components: Row,
    spawner: &'spawner mut Spawner<'world, 'state>,
}

impl<'world, 'state, 'spawner> Spawned<'world, 'state, 'spawner> {
    pub fn with<C: Component>(mut self, component: C) -> Self {
        let id = unsafe { self.spawner.world.components().get_id_unchecked::<C>() };
        self.components.insert(id, component);
        self
    }

    pub fn finish(self) -> Entity {
        let id = self.id;
        self.spawner.entities.push((id, self.components));
        id
    }
}
