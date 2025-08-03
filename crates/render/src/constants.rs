use crate::{GlobalShaderConstant, RenderDevice, processor::ShaderConstant};
use encase::ShaderType;

pub const MINIMUM_STORAGE_BUFFER_SIZE: u32 = 128 * 1024 * 1024;

pub const MAX_BATCH_SIZE: u32 = 512;

pub struct StorageBufferEnabled;
impl GlobalShaderConstant for StorageBufferEnabled {
    const NAME: &'static str = "STORAGE_BUFFER_ENABLED";

    fn get(device: &RenderDevice) -> ShaderConstant {
        let limits = device.limits();
        let enabled = limits.max_storage_buffers_per_shader_stage > 0
            && limits.max_storage_buffer_binding_size >= MINIMUM_STORAGE_BUFFER_SIZE;
        ShaderConstant::Bool(enabled)
    }
}

pub struct UniformBatchSize<T: ShaderType + 'static>(std::marker::PhantomData<T>);
impl<T: ShaderType + 'static> UniformBatchSize<T> {
    pub fn size(device: &RenderDevice, max: u32) -> u32 {
        (device.limits().max_uniform_buffer_binding_size / T::min_size().get() as u32).min(max)
    }
}

impl<T: ShaderType + 'static> GlobalShaderConstant for UniformBatchSize<T> {
    const NAME: &'static str = "BATCH_SIZE";

    fn get(device: &RenderDevice) -> ShaderConstant {
        ShaderConstant::U32(Self::size(device, MAX_BATCH_SIZE))
    }
}
