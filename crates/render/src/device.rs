use ecs::Resource;
use std::sync::Arc;
use wgpu::{Adapter, Device, Queue, RequestDeviceError};

#[derive(Resource, Clone)]
pub struct RenderDevice {
    device: Arc<Device>,
    pub queue: Arc<Queue>,
}

impl RenderDevice {
    pub async fn new(adapter: &Adapter) -> Result<Self, RequestDeviceError> {
        let desc = wgpu::DeviceDescriptor {
            required_features: wgpu::Features::ADDRESS_MODE_CLAMP_TO_BORDER
                | wgpu::Features::ADDRESS_MODE_CLAMP_TO_ZERO
                | wgpu::Features::INDIRECT_FIRST_INSTANCE
                | wgpu::Features::default(),
            ..Default::default()
        };

        let (device, queue) = adapter.request_device(&desc).await?;

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
