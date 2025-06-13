use ecs::Resource;
use std::sync::Arc;
use wgpu::{Adapter, Device, Queue, RequestDeviceError};

#[derive(Resource)]
pub struct RenderDevice {
    device: Arc<Device>,
    pub queue: Arc<Queue>,
}

impl RenderDevice {
    pub async fn new(adapter: &Adapter) -> Result<Self, RequestDeviceError> {
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor::default())
            .await?;

        Ok(Self {
            device: Arc::new(device),
            queue: Arc::new(queue),
        })
    }
}

impl std::ops::Deref for RenderDevice {
    type Target = Device;

    fn deref(&self) -> &Self::Target {
        &self.device
    }
}
