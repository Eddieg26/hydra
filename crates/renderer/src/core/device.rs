use std::sync::Arc;
use wgpu::{
    Adapter, Device, Features, Limits, MemoryHints, Queue, RequestDeviceError,
    wgt::DeviceDescriptor,
};

#[derive(Clone)]
pub struct RenderDevice {
    inner: Arc<Device>,
    queue: Arc<Queue>,
}

impl RenderDevice {
    pub async fn new(adapter: &Adapter) -> Result<Self, RequestDeviceError> {
        let (device, queue) = adapter
            .request_device(&DeviceDescriptor {
                label: None,
                required_features: Features::ADDRESS_MODE_CLAMP_TO_BORDER
                    | Features::ADDRESS_MODE_CLAMP_TO_ZERO
                    | Features::INDIRECT_FIRST_INSTANCE
                    | Features::TEXTURE_BINDING_ARRAY,
                required_limits: Limits::default(),
                memory_hints: MemoryHints::Performance,
                trace: wgpu::Trace::Off,
            })
            .await?;

        Ok(Self {
            inner: Arc::new(device),
            queue: Arc::new(queue),
        })
    }

    pub fn queue(&self) -> &Queue {
        &self.queue
    }
}

impl std::ops::Deref for RenderDevice {
    type Target = Device;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}
