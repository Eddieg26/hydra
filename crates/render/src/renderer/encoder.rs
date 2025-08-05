use crate::RenderDevice;
use ecs::SystemArg;
use wgpu::CommandEncoder;

pub struct RenderCommandEncoder<'w> {
    encoder: Option<CommandEncoder>,
    pub device: &'w RenderDevice,
}

impl<'w> std::ops::Deref for RenderCommandEncoder<'w> {
    type Target = CommandEncoder;

    fn deref(&self) -> &Self::Target {
        self.encoder.as_ref().unwrap()
    }
}

impl<'w> std::ops::DerefMut for RenderCommandEncoder<'w> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.encoder.as_mut().unwrap()
    }
}

unsafe impl SystemArg for RenderCommandEncoder<'_> {
    type Item<'world, 'state> = RenderCommandEncoder<'world>;

    type State = ();

    fn init(world: &mut ecs::World, access: &mut ecs::WorldAccess) -> Self::State {
        let id = world.register_resource::<RenderDevice>();
        access.resources_mut().reads(id);
        ()
    }

    unsafe fn get<'world, 'state>(
        _: &'state mut Self::State,
        world: ecs::world::WorldCell<'world>,
        _: &'world ecs::SystemMeta,
    ) -> Self::Item<'world, 'state> {
        let device = unsafe { world.get() }.resource::<RenderDevice>();
        let encoder = device.create_command_encoder(&Default::default());

        RenderCommandEncoder {
            encoder: Some(encoder),
            device,
        }
    }
}

impl Drop for RenderCommandEncoder<'_> {
    fn drop(&mut self) {
        if let Some(encoder) = self.encoder.take() {
            self.device.queue.submit(std::iter::once(encoder.finish()));
        }
    }
}
