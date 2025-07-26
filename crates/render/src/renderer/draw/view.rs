use ecs::{Component, Entity, Query, Resource, unlifetime::Read};
use std::collections::HashMap;
use transform::{GlobalTransform, LocalTransform};
use wgpu::{DynamicOffset, ShaderStages};

use crate::{
    BindGroup, BindGroupBuilder, BindGroupLayout, BindGroupLayoutBuilder, RenderDevice,
    RenderResource, uniform::UniformBufferArray,
};

pub trait View: Clone + Component {
    type Data: super::ShaderData;

    type Transform: LocalTransform;

    fn data(&self) -> Self::Data;
}

pub struct ViewInstance<V: View> {
    pub view: V,
    pub data: V::Data,
    pub local: V::Transform,
    pub global: GlobalTransform,
    pub offset: DynamicOffset,
}

#[derive(Resource)]
pub struct ViewSet<V: View>(pub(super) HashMap<Entity, ViewInstance<V>>);
impl<V: View> ViewSet<V> {
    pub fn new() -> Self {
        Self(HashMap::new())
    }

    pub(crate) fn extract(
        views: &mut Self,
        query: Query<(Entity, &GlobalTransform, &V::Transform, &V)>,
    ) {
        let mut extracted = HashMap::new();
        for (entity, global, local, view) in query.iter() {
            extracted.insert(
                entity,
                ViewInstance {
                    view: view.clone(),
                    data: view.data(),
                    local: *local,
                    global: *global,
                    offset: 0,
                },
            );
        }

        views.0 = extracted;
    }
}

#[derive(Resource)]
pub struct ViewBuffer<V: View> {
    buffer: UniformBufferArray<V::Data>,
    bind_group: BindGroup,
    layout: BindGroupLayout,
}

impl<V: View> ViewBuffer<V> {
    pub const BINDING: u32 = 0;

    pub fn layout(&self) -> &BindGroupLayout {
        &self.layout
    }

    pub fn bind_group(&self) -> &BindGroup {
        &self.bind_group
    }

    pub(crate) fn queue(views: &mut Self, extracted: &mut ViewSet<V>, device: &RenderDevice) {
        for view in extracted.0.values_mut() {
            view.offset = views.buffer.push(&view.data);
        }

        if let Some(buffer) = views.buffer.update(device).map(|_| views.buffer.as_ref()) {
            let bind_group = BindGroupBuilder::new(&views.layout)
                .with_uniform(Self::BINDING, &buffer, 0, None)
                .build(device);

            views.bind_group = bind_group
        }
    }
}

impl<V: View> AsRef<UniformBufferArray<V::Data>> for ViewBuffer<V> {
    fn as_ref(&self) -> &UniformBufferArray<V::Data> {
        &self.buffer
    }
}

impl<V: View> RenderResource for ViewBuffer<V> {
    type Arg = Read<RenderDevice>;

    fn extract(device: ecs::ArgItem<Self::Arg>) -> Result<Self, crate::ExtractError<()>> {
        let buffer = UniformBufferArray::new(device, None, None);
        let layout = BindGroupLayoutBuilder::new()
            .with_uniform(Self::BINDING, ShaderStages::all(), true, None, None)
            .build(device);

        let bind_group = BindGroupBuilder::new(&layout)
            .with_uniform(Self::BINDING, buffer.as_ref(), 0, None)
            .build(device);

        Ok(Self {
            buffer,
            bind_group,
            layout,
        })
    }
}
