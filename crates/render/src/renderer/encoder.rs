use crate::RenderDevice;
use ecs::SystemArg;
use wgpu::CommandEncoder;

pub struct RenderCommandEncoder<'w, 's> {
    encoder: &'s mut CommandEncoder,
    device: &'w RenderDevice,
}

impl<'w, 's> std::ops::Deref for RenderCommandEncoder<'w, 's> {
    type Target = CommandEncoder;

    fn deref(&self) -> &Self::Target {
        &self.encoder
    }
}

impl<'w, 's> std::ops::DerefMut for RenderCommandEncoder<'w, 's> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.encoder
    }
}

unsafe impl SystemArg for RenderCommandEncoder<'_, '_> {
    type Item<'world, 'state> = RenderCommandEncoder<'world, 'state>;

    type State = Option<CommandEncoder>;

    fn init(world: &mut ecs::World, access: &mut ecs::WorldAccess) -> Self::State {
        let id = world.register_resource::<RenderDevice>();
        access.resources_mut().reads(id);
        None
    }

    unsafe fn get<'world, 'state>(
        state: &'state mut Self::State,
        world: ecs::world::WorldCell<'world>,
        _: &'world ecs::SystemMeta,
    ) -> Self::Item<'world, 'state> {
        let device = unsafe { world.get() }.resource::<RenderDevice>();
        let encoder = device.create_command_encoder(&Default::default());

        state.replace(encoder);

        RenderCommandEncoder {
            encoder: state.as_mut().unwrap(),
            device,
        }
    }

    fn update(state: &mut Self::State, world: &mut ecs::World) {
        let device = world.resource::<RenderDevice>();
        if let Some(encoder) = state.take() {
            device.queue.submit(std::iter::once(encoder.finish()));
        }
    }
}
