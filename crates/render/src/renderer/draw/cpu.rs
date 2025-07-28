use crate::{
    BindGroup, BindGroupBuilder, BindGroupLayout, BindGroupLayoutBuilder, Buffer, MeshFormat,
    RenderAssets, RenderDevice, RenderMesh, RenderResource, ShaderData, SubMesh,
    drawable::{DrawCall, DrawMode, DrawPipeline, DrawSet, Drawable, ViewDrawSet},
    material::{Material, RenderPhase},
    view::ViewSet,
};
use ecs::{Resource, system::unlifetime::Read};
use encase::DynamicUniformBuffer;
use std::{collections::HashMap, num::NonZero, ops::Range};
use wgpu::{BufferUsages, ShaderStages};

pub const MAX_OBJECT_COUNT: u32 = 512;

#[derive(Debug)]
pub struct BatchIndex {
    pub bind_group: usize,
    pub instances: Range<u32>,
}

#[derive(Resource)]
pub struct UniformDataBuffer<T: ShaderData> {
    buffer: Buffer,
    data: DynamicUniformBuffer<Vec<u8>>,
    batch_size: u32,
    item_size: u32,
    layout: BindGroupLayout,
    bind_groups: Vec<BindGroup>,
    is_dirty: bool,
    _marker: std::marker::PhantomData<T>,
}

impl<T: ShaderData> UniformDataBuffer<T> {
    pub fn get_batch_size(device: &RenderDevice) -> u32 {
        (device.limits().max_uniform_buffer_binding_size / std::mem::size_of::<T>() as u32)
            .min(MAX_OBJECT_COUNT)
    }

    pub fn new(device: &RenderDevice) -> Self {
        let item_size = std::mem::size_of::<T>() as u32;
        let batch_size = Self::get_batch_size(device);

        let buffer = Buffer::new(
            device,
            (batch_size * item_size) as u64,
            BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            None,
        );

        let layout = BindGroupLayoutBuilder::new()
            .with_uniform(
                0,
                ShaderStages::VERTEX | ShaderStages::COMPUTE,
                false,
                None,
                None,
            )
            .build(device);

        let bind_group = BindGroupBuilder::new(&layout)
            .with_uniform(0, &buffer, 0, NonZero::new(buffer.size()))
            .build(device);

        Self {
            buffer,
            data: DynamicUniformBuffer::new_with_alignment(Vec::new(), item_size as u64),
            batch_size,
            item_size,
            layout,
            is_dirty: false,
            bind_groups: vec![bind_group],
            _marker: Default::default(),
        }
    }

    pub fn layout(&self) -> &BindGroupLayout {
        &self.layout
    }

    pub fn bind_groups(&self) -> &[BindGroup] {
        &self.bind_groups
    }

    pub fn batch_size(&self) -> u32 {
        self.batch_size
    }

    pub fn push(&mut self, values: &[T]) -> Vec<BatchIndex> {
        if values.is_empty() {
            return Vec::new();
        }

        let len = self.data.as_ref().len() as u32 / self.item_size;
        let start = len % self.batch_size;
        let initial = ((self.batch_size - start) as usize).min(values.len());
        let mut batches = vec![self.create_batch(start, &values[..initial])];
        for values in values[initial..].chunks(self.batch_size as usize) {
            batches.push(self.create_batch(0, values));
        }

        self.is_dirty = true;

        batches
    }

    fn create_batch(&mut self, start: u32, values: &[T]) -> BatchIndex {
        let bind_group = self.data.as_ref().len() / (self.batch_size * self.item_size) as usize;
        let instances = start..start + values.len() as u32;

        for value in values {
            self.data.write(value).unwrap();
        }

        BatchIndex {
            bind_group,
            instances,
        }
    }

    pub fn update(&mut self, device: &RenderDevice) {
        if self.data.as_ref().len() as u64 > self.buffer.size() {
            let len = self.data.as_ref().len() as u32 / self.item_size;
            let padding = self.batch_size - (len % self.batch_size);
            let new_len = padding + len;
            let batch_count = new_len / self.batch_size;

            self.buffer
                .resize(device, (new_len * self.item_size) as u64);
            self.create_bind_groups(device, batch_count);
        }

        if self.is_dirty && self.data.as_ref().len() > 0 {
            self.buffer.update(device, self.data.as_ref().as_slice());
            self.is_dirty = false;
        }
    }

    fn create_bind_groups(&mut self, device: &RenderDevice, count: u32) {
        let mut bind_groups = Vec::with_capacity(count as usize);
        let batch_size = self.batch_size * self.item_size;
        for index in 0..count {
            let offset = index * batch_size;
            let size = NonZero::new(batch_size as u64);
            let bind_group = BindGroupBuilder::new(&self.layout)
                .with_uniform(0, &self.buffer, offset as u64, size)
                .build(device);
            bind_groups.push(bind_group);
        }

        self.bind_groups = bind_groups;
    }

    pub(crate) fn update_buffer(device: &RenderDevice, data: &mut Self) {
        data.update(device);
    }

    pub(crate) fn clear_buffer(data: &mut Self) {
        data.data.as_mut().clear();
        data.data.set_offset(0);
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

impl<T: ShaderData> RenderResource for UniformDataBuffer<T> {
    type Arg = Read<RenderDevice>;

    fn extract(device: ecs::ArgItem<Self::Arg>) -> Result<Self, crate::ExtractError<()>> {
        Ok(UniformDataBuffer::new(device))
    }
}

impl<T: ShaderData> AsRef<Buffer> for UniformDataBuffer<T> {
    fn as_ref(&self) -> &Buffer {
        &self.buffer
    }
}
