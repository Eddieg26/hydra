use crate::{
    BindGroup, BindGroupBuilder, BindGroupLayout, BindGroupLayoutBuilder, BlendMode, Camera,
    Frustum, Projection, RenderDevice, RenderResource, RenderSurface, uniform::UniformBufferArray,
};
use ecs::{
    Component, Entity, Resource,
    app::Main,
    system::unlifetime::{Read, SQuery},
};
use encase::{ShaderType, internal::WriteInto};
use math::Mat4;
use std::collections::HashMap;
use transform::{GlobalTransform, LocalTransform};
use wgpu::ShaderStages;

pub trait ViewData: ShaderType + WriteInto + Send + Sync + 'static {
    fn projection(&self) -> Mat4;
}

pub trait View: Component + Clone {
    type Data: ViewData;

    type Transform: LocalTransform;

    type Item: Default + Copy + PartialOrd + Send + Sync + 'static;

    fn data(&self, aspect_ratio: f32, camera: &Camera, transform: &GlobalTransform) -> Self::Data;

    fn item(
        &self,
        data: &Self::Data,
        mode: BlendMode,
        local_transform: &Self::Transform,
        global_transform: &GlobalTransform,
    ) -> Self::Item;

    fn projection(&self) -> Projection;
}

pub struct RenderView<V: View> {
    pub view: V,
    pub data: V::Data,
    pub transform: GlobalTransform,
    pub frustum: Frustum,
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
pub struct RenderViews<V: View> {
    views: HashMap<Entity, RenderView<V>>,
    buffer: UniformBufferArray<V::Data>,
    layout: BindGroupLayout,
    bind_group: Option<BindGroup>,
}

impl<V: View> RenderViews<V> {
    pub const BINDING: u32 = 0;

    pub fn new(device: &RenderDevice) -> Self {
        let buffer = UniformBufferArray::new();

        let layout = BindGroupLayoutBuilder::new()
            .with_uniform(Self::BINDING, ShaderStages::all(), true, None, None)
            .build(device);

        Self {
            views: HashMap::new(),
            buffer,
            layout,
            bind_group: None,
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = (&Entity, &RenderView<V>)> {
        self.views.iter()
    }

    pub fn get(&self, entity: &Entity) -> Option<&RenderView<V>> {
        self.views.get(entity)
    }

    pub fn layout(&self) -> &BindGroupLayout {
        &self.layout
    }

    pub fn bind_group(&self) -> Option<&BindGroup> {
        self.bind_group.as_ref()
    }

    pub fn len(&self) -> usize {
        self.views.len()
    }

    pub(crate) fn extract(
        buffer: &mut Self,
        query: Main<SQuery<(Entity, &V, &GlobalTransform, &Camera)>>,
        surface: &RenderSurface,
    ) {
        buffer.as_mut().clear();

        let aspect_ratio = surface.width() as f32 / surface.height() as f32;

        buffer.views = query
            .iter()
            .map(|(entity, view, transform, camera)| {
                let data = view.data(aspect_ratio, camera, transform);
                let dynamic_offset = buffer.as_mut().push(&data);
                let clip_from_world = data.projection() * transform.matrix().inverse();

                let frustum = Frustum::from_world_projection(
                    &clip_from_world,
                    &transform.translation(),
                    &transform.back(),
                    view.projection().far(),
                );

                let view = RenderView {
                    view: view.clone(),
                    data,
                    frustum,
                    transform: *transform,
                    dynamic_offset,
                };

                (entity, view)
            })
            .collect::<HashMap<_, _>>();
    }

    pub(crate) fn queue(views: &mut Self, device: &RenderDevice) {
        if let Some(buffer) = views
            .as_mut()
            .update(device)
            .and_then(|_| views.buffer.inner())
        {
            let bind_group = BindGroupBuilder::new(&views.layout)
                .with_uniform(Self::BINDING, &buffer, 0, None)
                .build(device);

            views.bind_group = Some(bind_group);
        }
    }
}

impl<V: View> RenderResource for RenderViews<V> {
    type Arg = Read<RenderDevice>;

    fn extract(device: ecs::ArgItem<Self::Arg>) -> Result<Self, crate::ExtractError<()>> {
        Ok(Self::new(device))
    }
}

impl<V: View> AsRef<UniformBufferArray<V::Data>> for RenderViews<V> {
    fn as_ref(&self) -> &UniformBufferArray<V::Data> {
        &self.buffer
    }
}

impl<V: View> AsMut<UniformBufferArray<V::Data>> for RenderViews<V> {
    fn as_mut(&mut self) -> &mut UniformBufferArray<V::Data> {
        &mut self.buffer
    }
}
