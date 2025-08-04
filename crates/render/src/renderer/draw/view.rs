use crate::{
    BindGroup, BindGroupBuilder, BindGroupLayout, BindGroupLayoutBuilder, RenderDevice,
    RenderResource, RenderSurface, uniform::UniformBufferArray,
};
use ecs::{Component, Entity, Query, Resource, unlifetime::Read};
use encase::ShaderType;
use math::{Mat4, Size, Vec3};
use std::collections::HashMap;
use transform::{GlobalTransform, LocalTransform};
use wgpu::{DynamicOffset, ShaderStages};

pub trait View: Clone + Component {
    type Transform: LocalTransform;

    type Item: Default + Copy + PartialOrd + PartialEq + Send + Sync + 'static;

    fn item(&self, _local: &Self::Transform, _global: &GlobalTransform) -> Self::Item {
        Self::Item::default()
    }

    fn projection(&self, width: f32, height: f32) -> Mat4;

    fn far(&self) -> f32;
}

#[derive(Debug, Clone, Copy, ShaderType)]
pub struct ViewData {
    pub world: Mat4,
    pub view: Mat4,
    pub projection: Mat4,
}

impl ViewData {
    pub fn new(world: GlobalTransform, projection: Mat4) -> Self {
        let (_, rotation, translation) = world.to_scale_rotation_translation();
        let view = Mat4::look_at_rh(
            translation,
            translation + rotation * Vec3::Z,
            rotation * Vec3::Y,
        );

        Self {
            world: world.matrix(),
            view,
            projection,
        }
    }
}

pub struct ViewInstance<V: View> {
    pub offset: DynamicOffset,
    _marker: std::marker::PhantomData<V>,
}

impl<V: View> Copy for ViewInstance<V> {}
impl<V: View> Clone for ViewInstance<V> {
    fn clone(&self) -> Self {
        Self {
            offset: self.offset.clone(),
            _marker: self._marker.clone(),
        }
    }
}

#[derive(Resource)]
pub struct ViewBuffer<V: View> {
    instances: HashMap<Entity, ViewInstance<V>>,
    buffer: UniformBufferArray<ViewData>,
    layout: BindGroupLayout,
    bind_group: BindGroup,
    _marker: std::marker::PhantomData<V>,
}

impl<V: View> ViewBuffer<V> {
    pub fn instance(&self, view: &Entity) -> Option<ViewInstance<V>> {
        self.instances.get(view).copied()
    }

    pub fn layout(&self) -> &BindGroupLayout {
        &self.layout
    }

    pub fn bind_group(&self) -> &BindGroup {
        &self.bind_group
    }

    pub fn clear(&mut self) {
        self.buffer.clear();
        self.instances.clear();
    }

    pub fn queue(
        views: &mut Self,
        query: Query<(Entity, &GlobalTransform, &V)>,
        surface: &RenderSurface,
        device: &RenderDevice,
    ) {
        views.clear();

        let Size { width, height } = surface.size();
        for (entity, transform, view) in query.iter() {
            let projection = view.projection(width as f32, height as f32);
            let data = ViewData::new(*transform, projection);
            let offset = ViewInstance {
                offset: views.buffer.push(&data),
                _marker: std::marker::PhantomData,
            };
            views.instances.insert(entity, offset);
        }

        if let Some(buffer) = views.buffer.update(device).map(|_| views.buffer.as_ref()) {
            let bind_group = BindGroupBuilder::new(&views.layout)
                .with_uniform(0, &buffer, 0, None)
                .build(device);

            views.bind_group = bind_group
        }
    }
}

impl<V: View> AsRef<UniformBufferArray<ViewData>> for ViewBuffer<V> {
    fn as_ref(&self) -> &UniformBufferArray<ViewData> {
        &self.buffer
    }
}

impl<V: View> RenderResource for ViewBuffer<V> {
    type Arg = Read<RenderDevice>;

    fn extract(device: ecs::ArgItem<Self::Arg>) -> Result<Self, crate::ExtractError<()>> {
        let buffer = UniformBufferArray::new(device, None, None);

        let layout = BindGroupLayoutBuilder::new()
            .with_uniform(0, ShaderStages::all(), true, None, None)
            .build(device);

        let binding = BindGroupBuilder::new(&layout)
            .with_uniform(0, buffer.as_ref(), 0, None)
            .build(device);

        Ok(Self {
            instances: HashMap::new(),
            buffer,
            layout,
            bind_group: binding,
            _marker: std::marker::PhantomData,
        })
    }
}
