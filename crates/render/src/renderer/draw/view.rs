use crate::{
    BindGroup, BindGroupBuilder, BindGroupLayout, BindGroupLayoutBuilder, BlendMode, Camera,
    RenderDevice, RenderResource, RenderSurface, uniform::UniformBufferArray,
};
use ecs::{
    Component, Entity, Resource,
    app::Main,
    system::unlifetime::{Read, SQuery},
};
use encase::{ShaderType, internal::WriteInto};
use std::collections::HashMap;
use transform::{GlobalTransform, LocalTransform};
use wgpu::{BufferUsages, ShaderStages};

pub trait View: Component + Clone {
    type Data: ShaderType + WriteInto + Send + Sync + 'static;

    type Transform: LocalTransform;

    type Item: Default + Copy + PartialOrd + Send + Sync + 'static;

    fn data(
        &self,
        screen_width: u32,
        screen_height: u32,
        camera: &Camera,
        transform: &GlobalTransform,
    ) -> Self::Data;

    fn item(
        &self,
        data: &Self::Data,
        mode: BlendMode,
        local_transform: &Self::Transform,
        global_transform: &GlobalTransform,
    ) -> Self::Item;
}

pub struct RenderView<V: View> {
    pub view: V,
    pub data: V::Data,
    pub transform: GlobalTransform,
    pub dynamic_offset: u32,
}

impl<V: View> RenderView<V> {
    pub fn item(
        &self,
        mode: BlendMode,
        local_transform: &V::Transform,
        global_transform: &GlobalTransform,
    ) -> V::Item {
        self.view
            .item(&self.data, mode, local_transform, global_transform)
    }
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

    pub fn views(&self) -> impl Iterator<Item = (&Entity, &RenderView<V>)> {
        self.views.iter()
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
        surface: &RenderSurface,
    ) {
        buffer.as_mut().clear();

        buffer.views = query
            .iter()
            .map(|(entity, view, transform, camera)| {
                let data = view.data(surface.width(), surface.height(), camera, transform);
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

impl<V: View> RenderResource for ViewDataBuffer<V> {
    type Arg = Read<RenderDevice>;

    fn extract(device: ecs::ArgItem<Self::Arg>) -> Result<Self, crate::ExtractError<()>> {
        Ok(Self::new(device))
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
