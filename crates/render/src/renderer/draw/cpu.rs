use crate::{
    BindGroup, BindGroupBuilder, BindGroupLayout, BindGroupLayoutBuilder, MeshFormat, RenderAssets,
    RenderDevice, RenderMesh, RenderResource, SubMesh,
    drawable::{DrawCall, DrawMode, DrawPipeline, DrawSet, Drawable, ViewDrawSet},
    material::{Material, RenderPhase},
    uniform::UniformBufferArray,
    view::ViewSet,
};
use ecs::{Resource, system::unlifetime::Read};
use std::{collections::HashMap, num::NonZero, ops::Range};
use wgpu::ShaderStages;

pub const MAX_OBJECT_COUNT: u32 = 512;

#[derive(Debug)]
pub struct BatchIndex {
    pub bind_group: usize,
    pub instances: Range<u32>,
}

#[derive(Resource)]
pub struct UniformDataBuffer<T: super::ShaderData> {
    buffer: UniformBufferArray<T>,
    layout: BindGroupLayout,
    bind_groups: Vec<BindGroup>,
    item_size: u32,  // Size of a single item in the buffer
    batch_size: u32, // Size of a single batch in bytes
}

impl<T: super::ShaderData> UniformDataBuffer<T> {
    pub fn get_object_count(device: &RenderDevice) -> u32 {
        let item_size = T::min_size().get() as u32;

        (device.limits().max_uniform_buffer_binding_size / item_size).min(MAX_OBJECT_COUNT)
    }

    pub fn new(device: &RenderDevice) -> Self {
        let item_size = T::min_size().get() as u32;

        let batch_size = Self::get_object_count(device) * item_size;

        let buffer = UniformBufferArray::with_alignment(device, batch_size, None, None);

        let layout = BindGroupLayoutBuilder::new()
            .with_label(ecs::ext::short_type_name::<Self>())
            .with_uniform(
                0,
                ShaderStages::VERTEX | ShaderStages::COMPUTE,
                false,
                None,
                None,
            )
            .build(device);

        let bind_group = BindGroupBuilder::new(&layout)
            .with_uniform(0, buffer.as_ref(), 0, NonZero::new(batch_size as u64))
            .build(device);

        Self {
            buffer,
            layout,
            bind_groups: vec![bind_group],
            item_size,
            batch_size,
        }
    }

    pub fn push<'a>(&mut self, values: &[T]) -> Vec<BatchIndex> {
        if values.is_empty() {
            return Vec::new();
        }

        let batch_count = self.batch_size / self.item_size;
        let current_batch_count = ((self.buffer.data().len() % self.batch_size as usize)
            / self.item_size as usize) as u32;
        let remaining = (batch_count - current_batch_count).min(values.len() as u32) as usize;
        let initial =
            self.create_batch(current_batch_count, remaining as u32, &values[0..remaining]);

        let mut batches = vec![initial];
        for values in values[remaining..].chunks(batch_count as usize) {
            let batch = self.create_batch(0, batch_count, values);
            batches.push(batch);
        }

        batches
    }

    #[inline]
    fn create_batch(&mut self, offset: u32, batch_count: u32, batch: &[T]) -> BatchIndex {
        let bind_group = self.buffer.data().len() / self.batch_size as usize;
        let instances = offset..offset + batch_count;
        for value in batch {
            self.buffer.push(value);
        }

        BatchIndex {
            bind_group,
            instances,
        }
    }

    pub fn update(&mut self, device: &RenderDevice) {
        let Some(buffer_size) = self.buffer.update(device) else {
            return;
        };

        let new_capacity = buffer_size as usize / self.batch_size as usize
            + (buffer_size as usize % self.batch_size as usize).min(1);
        let mut bind_groups = Vec::with_capacity(new_capacity);

        for index in 0..new_capacity {
            let offset = index as u64 * self.batch_size as u64;
            let size = NonZero::new((buffer_size - offset).min(self.batch_size as u64));
            let bind_group = BindGroupBuilder::new(&self.layout)
                .with_uniform(0, self.buffer.as_ref(), offset, size)
                .build(device);
            bind_groups.push(bind_group);
        }

        self.bind_groups = bind_groups;
    }

    pub fn layout(&self) -> &BindGroupLayout {
        &self.layout
    }

    pub fn bind_groups(&self) -> &[BindGroup] {
        &self.bind_groups
    }

    pub fn item_size(&self) -> u32 {
        self.item_size
    }

    pub fn batch_size(&self) -> u32 {
        self.batch_size
    }

    pub fn clear(&mut self) {
        self.buffer.clear();
    }

    pub(crate) fn update_buffer(device: &RenderDevice, data: &mut Self) {
        data.update(device);
    }

    pub(crate) fn clear_buffer(data: &mut Self) {
        data.clear();
    }

    pub(crate) fn queue<D, P>(
        views: &ViewSet<D::View>,
        drawables: &DrawSet<D>,
        pipeline: &DrawPipeline<D>,
        meshes: &RenderAssets<RenderMesh>,
        sub_meshes: &RenderAssets<SubMesh>,
        view_draw_set: &mut ViewDrawSet<D::View, P>,
        instances: &mut Self,
    ) where
        P: RenderPhase<View = D::View>,
        D: Drawable<Model = T>,
        D::Material: Material<Phase = P>,
    {
        for view in views.0.keys() {
            let mut batches = HashMap::new();

            for drawable in &drawables.0 {
                batches
                    .entry(drawable.key)
                    .or_insert(Vec::new())
                    .push(drawable);
            }

            for (key, drawables) in batches {
                let Some(mesh) = meshes.get(&key.mesh) else {
                    continue;
                };

                if mesh.key() != pipeline.key() {
                    continue;
                }

                let sub_mesh = match key.sub_mesh {
                    Some(id) => sub_meshes.get(&id).copied(),
                    None => Some(SubMesh::from(mesh)),
                };

                let Some(sub_mesh) = sub_mesh else {
                    continue;
                };

                let data = drawables.iter().map(|d| d.model()).collect::<Vec<_>>();
                let mut batches = instances.push(&data);

                match mesh.format() {
                    MeshFormat::Indexed { format, .. } => view_draw_set
                        .0
                        .entry(*view)
                        .or_default()
                        .extend(batches.drain(..).map(|b| DrawCall {
                            material: key.material,
                            mesh: key.mesh,
                            sub_mesh,
                            mode: DrawMode::IndexedDirect {
                                bind_group: b.bind_group,
                                instances: b.instances,
                                format,
                            },
                            pipeline: **pipeline,
                            item: P::Item::default(),
                            function: ViewDrawSet::<D::View, P>::draw::<D>,
                        })),
                    MeshFormat::NonIndexed => {
                        view_draw_set
                            .0
                            .entry(*view)
                            .or_default()
                            .extend(batches.drain(..).map(|b| DrawCall {
                                material: key.material,
                                mesh: key.mesh,
                                sub_mesh,
                                mode: DrawMode::Direct {
                                    bind_group: b.bind_group,
                                    instances: b.instances,
                                },
                                pipeline: **pipeline,
                                item: P::Item::default(),
                                function: ViewDrawSet::<D::View, P>::draw::<D>,
                            }))
                    }
                };
            }
        }
    }
}

impl<T: super::ShaderData> RenderResource for UniformDataBuffer<T> {
    type Arg = Read<RenderDevice>;

    fn extract(device: ecs::ArgItem<Self::Arg>) -> Result<Self, crate::ExtractError<()>> {
        Ok(UniformDataBuffer::new(device))
    }
}

impl<T: super::ShaderData> AsRef<UniformBufferArray<T>> for UniformDataBuffer<T> {
    fn as_ref(&self) -> &UniformBufferArray<T> {
        &self.buffer
    }
}
