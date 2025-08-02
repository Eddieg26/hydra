use crate::{
    BindGroup, BindGroupBuilder, BindGroupLayout, BindGroupLayoutBuilder, RenderDevice,
    RenderResource, RenderSurface, uniform::UniformBufferArray,
};
use ecs::{
    Component, Entity, Resource,
    system::Main,
    unlifetime::{Read, SQuery},
};
use encase::ShaderType;
use math::{Mat4, Size, Vec3};
use std::collections::HashMap;
use transform::{GlobalTransform, LocalTransform};
use wgpu::{DynamicOffset, ShaderStages};

pub trait View: Clone + Component {
    type Transform: LocalTransform;

    type Item: Default + Copy + Ord + Eq + Send + Sync + 'static;

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
    pub fn new(world: Mat4, projection: Mat4) -> Self {
        let (_, rotation, translation) = world.to_scale_rotation_translation();
        let view = Mat4::look_at_rh(
            translation,
            translation + rotation * Vec3::Z,
            rotation * Vec3::Y,
        );

        Self {
            world,
            view,
            projection,
        }
    }
}

pub struct ViewInstance<V: View> {
    pub view: V,
    pub data: ViewData,
    pub offset: DynamicOffset,
}

#[derive(Resource)]
pub struct ViewSet<V: View>(pub(crate) HashMap<Entity, ViewInstance<V>>);
impl<V: View> ViewSet<V> {
    pub fn new() -> Self {
        Self(HashMap::new())
    }

    pub fn get(&self, entity: &Entity) -> Option<&ViewInstance<V>> {
        self.0.get(entity)
    }

    pub fn extract(
        set: &mut Self,
        surface: &RenderSurface,
        views: Main<SQuery<(Entity, &GlobalTransform, &V)>>,
    ) {
        let Size { width, height } = surface.size();
        let mut instances = HashMap::new();
        for (entity, transform, view) in views.iter() {
            let projection = view.projection(width as f32, height as f32);
            let data = ViewData::new(transform.matrix(), projection);
            instances.insert(
                entity,
                ViewInstance {
                    view: view.clone(),
                    data,
                    offset: 0,
                },
            );
        }

        set.0 = instances;
    }
}

#[derive(Resource)]
pub struct ViewBuffer<V: View> {
    buffer: UniformBufferArray<ViewData>,
    layout: BindGroupLayout,
    bind_group: BindGroup,
    _marker: std::marker::PhantomData<V>,
}

impl<V: View> ViewBuffer<V> {
    pub fn layout(&self) -> &BindGroupLayout {
        &self.layout
    }

    pub fn bind_group(&self) -> &BindGroup {
        &self.bind_group
    }

    pub fn queue(views: &mut Self, set: &mut ViewSet<V>, device: &RenderDevice) {
        views.buffer.reset();
        
        for view in set.0.values_mut() {
            view.offset = views.buffer.push(&view.data);
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
            buffer,
            layout,
            bind_group: binding,
            _marker: std::marker::PhantomData,
        })
    }
}
