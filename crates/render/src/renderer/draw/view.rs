use crate::{
    BindGroup, BindGroupBuilder, BindGroupLayout, BindGroupLayoutBuilder, Camera, RenderDevice,
    uniform::UniformBufferArray,
};
use ecs::{Component, Entity, Resource, app::Main, system::unlifetime::SQuery};
use encase::{ShaderType, internal::WriteInto};
use std::collections::HashMap;
use transform::{GlobalTransform, LocalTransform};
use wgpu::{BufferUsages, ShaderStages};

pub trait View: Component + Clone {
    type Data: ShaderType + WriteInto;

    type Transform: LocalTransform;

    type Key: Copy + Eq + Ord + Send + Sync + 'static;

    fn data(&self, camera: &Camera, transform: &GlobalTransform) -> Self::Data;

    fn key(
        &self,
        data: &Self::Data,
        local_entity_transform: &Self::Transform,
        global_entity_transform: &GlobalTransform,
    ) -> Self::Key;
}

pub struct RenderView<V: View> {
    pub view: V,
    pub data: V::Data,
    pub transform: GlobalTransform,
    pub dynamic_offset: u32,
}

#[derive(Resource)]
pub struct ViewDataBuffer<V: View> {
    views: HashMap<Entity, RenderView<V>>,
    buffer: UniformBufferArray<V::Data>,
    layout: BindGroupLayout,
    bind_group: BindGroup,
}

impl<V: View> ViewDataBuffer<V> {
    pub const BINDING: u32 = 0;

    pub fn new(device: &RenderDevice) -> Self {
        let buffer = UniformBufferArray::new(device, None, Some(BufferUsages::COPY_DST));

        let layout = BindGroupLayoutBuilder::new()
            .with_uniform(Self::BINDING, ShaderStages::all(), true, None, None)
            .build(device);

        let bind_group = BindGroupBuilder::new(&layout)
            .with_uniform(Self::BINDING, &buffer, 0, None)
            .build(device);

        Self {
            views: HashMap::new(),
            buffer,
            layout,
            bind_group,
        }
    }

    pub fn get(&self, entity: &Entity) -> Option<&RenderView<V>> {
        self.views.get(entity)
    }

    pub fn layout(&self) -> &BindGroupLayout {
        &self.layout
    }

    pub fn bind_group(&self) -> &BindGroup {
        &self.bind_group
    }

    pub(crate) fn extract(
        buffer: &mut Self,
        query: Main<SQuery<(Entity, &V, &GlobalTransform, &Camera)>>,
    ) {
        buffer.as_mut().clear();

        buffer.views = query
            .iter()
            .map(|(entity, view, transform, camera)| {
                let data = view.data(camera, transform);
                let dynamic_offset = buffer.as_mut().push(&data);
                let view = RenderView {
                    view: view.clone(),
                    data,
                    transform: *transform,
                    dynamic_offset,
                };

                (entity, view)
            })
            .collect::<HashMap<_, _>>();
    }

    pub(crate) fn process(buffer: &mut Self, device: &RenderDevice) {
        if buffer.as_mut().update(device).is_some() {
            buffer.bind_group = BindGroupBuilder::new(&buffer.layout)
                .with_uniform(Self::BINDING, buffer.as_ref(), 0, None)
                .build(device);
        }
    }
}

impl<V: View> AsRef<UniformBufferArray<V::Data>> for ViewDataBuffer<V> {
    fn as_ref(&self) -> &UniformBufferArray<V::Data> {
        &self.buffer
    }
}

impl<V: View> AsMut<UniformBufferArray<V::Data>> for ViewDataBuffer<V> {
    fn as_mut(&mut self) -> &mut UniformBufferArray<V::Data> {
        &mut self.buffer
    }
}
