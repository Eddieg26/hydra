use crate::{GlobalShaderConstant, RenderDevice, shader};

pub const MINIMUM_STORAGE_BUFFER_SIZE: u32 = 128 * 1024 * 1024;

pub struct StorageBufferEnabled;
impl GlobalShaderConstant for StorageBufferEnabled {
    const NAME: &'static str = "STORAGE_BUFFER_ENABLED";

    fn get(device: &RenderDevice) -> shader::processor::ShaderConstant {
        let limits = device.limits();
        let enabled = limits.max_storage_buffers_per_shader_stage > 0
            && limits.max_storage_buffer_binding_size >= MINIMUM_STORAGE_BUFFER_SIZE;
        shader::processor::ShaderConstant::Bool(enabled)
    }
}
