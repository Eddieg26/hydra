use ecs::Component;
use math::Mat4;
use render::{
    BindGroup, BindGroupBuilder, BindGroupLayout, BindGroupLayoutBuilder, Projection, RenderDevice,
    ShaderType,
    uniform::UniformBufferArray,
    wgpu::{DynamicOffset, ShaderStages},
};

#[derive(Clone, Copy, Component)]
pub struct View3d {
    pub projection: Projection,
}

#[derive(ShaderType)]
pub struct View3dUniform {
    pub world: Mat4,
    pub view: Mat4,
    pub projection: Mat4,
}

#[derive(Clone, Copy, Component)]
pub struct ViewOffset(pub DynamicOffset);

pub struct View3dBuffer {
    buffer: UniformBufferArray<View3dUniform>,
    bind_group_layout: BindGroupLayout,
    bind_group: BindGroup,
}

impl View3dBuffer {
    pub fn new(device: &RenderDevice) -> Self {
        let buffer = UniformBufferArray::new(device, None, None);

        let bind_group_layout = BindGroupLayoutBuilder::new()
            .with_uniform(0, ShaderStages::all(), true, None, None)
            .build(device);

        let bind_group = BindGroupBuilder::new(&bind_group_layout)
            .with_uniform(0, &buffer, 0, None)
            .build(device);

        Self {
            buffer,
            bind_group_layout,
            bind_group,
        }
    }

    pub fn bind_group_layout(&self) -> &BindGroupLayout {
        &self.bind_group_layout
    }

    pub fn bind_group(&self) -> &BindGroup {
        &self.bind_group
    }

    pub fn push(&mut self, view: &View3dUniform) -> ViewOffset {
        ViewOffset(self.buffer.push(view))
    }

    pub fn update(&mut self, device: &RenderDevice) {
        if let Some(_) = self.buffer.update(device) {
            self.bind_group = BindGroupBuilder::new(&self.bind_group_layout)
                .with_uniform(0, &self.buffer, 0, None)
                .build(device)
        }
    }
}

impl std::ops::Deref for View3dBuffer {
    type Target = UniformBufferArray<View3dUniform>;

    fn deref(&self) -> &Self::Target {
        &self.buffer
    }
}

impl AsRef<UniformBufferArray<View3dUniform>> for View3dBuffer {
    fn as_ref(&self) -> &UniformBufferArray<View3dUniform> {
        &self.buffer
    }
}
