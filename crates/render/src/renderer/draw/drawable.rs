use crate::{
    ExtractResource, FragmentState, GlobalShaderConstant, GlobalShaderConstants, Mesh, MeshFilter,
    MeshKey, MeshLayout, PipelineCache, PipelineId, RenderAssets, RenderPipelineDesc,
    RenderResource, RenderState, RenderSurface, Shader, ShaderData, SubMesh, VertexState,
    allocator::MeshAllocator,
    constants::StorageBufferEnabled,
    cpu::UniformDataBuffer,
    gpu::{DrawArgsBuffer, StorageDataBuffer},
    material::{DepthWrite, Material, MaterialInstance, MaterialLayout, RenderPhase},
    processor::ShaderConstant,
    view::{View, ViewBuffer, ViewInstance},
};
use asset::{AssetId, ErasedId};
use ecs::{
    Component, Entity, Resource, World,
    system::Main,
    unlifetime::{Read, SCommands, SQuery, Write},
};
use std::{collections::HashMap, ops::Range};
use transform::GlobalTransform;
use wgpu::{ColorTargetState, IndexFormat, VertexStepMode};

pub trait Drawable: Clone + Component + 'static {
    type View: View;

    type Model: ShaderData;

    type Material: Material<View = Self::View>;

    fn material(&self) -> AssetId<Self::Material>;

    fn model(&self, transform: &GlobalTransform) -> Self::Model;

    fn primitive() -> wgpu::PrimitiveState {
        wgpu::PrimitiveState::default()
    }

    fn format() -> &'static [wgpu::VertexFormat];

    fn shader() -> impl Into<AssetId<Shader>>;
}

#[derive(Resource)]
pub struct DrawSet<D: Drawable>(pub(super) Vec<DrawInstance<D>>);

impl<D: Drawable> DrawSet<D> {
    pub fn new() -> Self {
        Self(Vec::new())
    }

    pub(crate) fn extract(
        drawables: &mut Self,
        query: Main<
            SQuery<(
                Entity,
                &D,
                &<D::View as View>::Transform,
                &GlobalTransform,
                &MeshFilter,
            )>,
        >,
    ) {
        let mut extracted = Vec::with_capacity(drawables.0.capacity());
        for (entity, draw, local, global, filter) in query.iter() {
            extracted.push(DrawInstance {
                entity,
                key: BatchKey {
                    material: draw.material().into(),
                    mesh: filter.mesh,
                    sub_mesh: filter.sub_mesh,
                },
                draw: draw.clone(),
                local: *local,
                global: *global,
            });
        }

        drawables.0 = extracted;
    }

    pub fn clear(&mut self) {
        self.0.clear();
    }

    pub(crate) fn clear_set(set: &mut Self) {
        set.clear();
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BatchKey {
    pub material: ErasedId,
    pub mesh: AssetId<Mesh>,
    pub sub_mesh: Option<AssetId<SubMesh>>,
}

pub struct DrawInstance<D: Drawable> {
    pub entity: Entity,
    pub key: BatchKey,
    pub draw: D,
    pub local: <D::View as View>::Transform,
    pub global: GlobalTransform,
}

impl<D: Drawable> DrawInstance<D> {
    pub fn model(&self) -> D::Model {
        self.draw.model(&self.global)
    }
}

#[derive(Resource)]
pub struct DrawPipeline<D: Drawable> {
    pipeline: PipelineId,
    key: MeshKey,
    _marker: std::marker::PhantomData<D>,
}

impl<D: Drawable> DrawPipeline<D> {
    pub fn new(pipeline: PipelineId, key: MeshKey) -> Self {
        Self {
            pipeline,
            key,
            _marker: std::marker::PhantomData,
        }
    }

    pub fn key(&self) -> MeshKey {
        self.key
    }
}

impl<D: Drawable> std::ops::Deref for DrawPipeline<D> {
    type Target = PipelineId;

    fn deref(&self) -> &Self::Target {
        &self.pipeline
    }
}
impl<D: Drawable> Copy for DrawPipeline<D> {}
impl<D: Drawable> Clone for DrawPipeline<D> {
    fn clone(&self) -> Self {
        Self {
            pipeline: self.pipeline,
            key: self.key,
            _marker: std::marker::PhantomData,
        }
    }
}

impl<D: Drawable> RenderResource for DrawPipeline<D> {
    type Arg = (
        Write<PipelineCache>,
        Read<RenderSurface>,
        Read<GlobalShaderConstants>,
        Option<Read<UniformDataBuffer<D::Model>>>,
        Option<Read<StorageDataBuffer<D::Model>>>,
        Option<Read<ViewBuffer<D::View>>>,
        Option<Write<MaterialLayout<D::Material>>>,
        SCommands,
    );

    fn extract(arg: ecs::ArgItem<Self::Arg>) -> Result<Self, crate::ExtractError<()>> {
        let (cache, surface, constants, cpu_model, gpu_model, views, layout, mut commands) = arg;

        let view_layout = match views {
            Some(views) => views.layout(),
            None => return Err(crate::resources::ExtractError::Retry(())),
        };

        let model_layout =
            if let Some(ShaderConstant::Bool(true)) = constants.get(StorageBufferEnabled::NAME) {
                gpu_model.map(|v| v.layout())
            } else {
                cpu_model.map(|v| v.layout())
            };

        let Some(model_layout) = model_layout else {
            return Err(crate::resources::ExtractError::Retry(()));
        };

        let material_layout = match layout {
            Some(layout) => layout,
            None => {
                commands.add(ExtractResource::<MaterialLayout<D::Material>>::new());
                return Err(crate::resources::ExtractError::Retry(()));
            }
        };

        let vertex_shader: AssetId<Shader> = D::shader().into();
        let fragment_shader: AssetId<Shader> = D::Material::shader().into();

        assert!(
            !D::format().is_empty(),
            "Mesh pipeline must have at least one vertex format"
        );

        let buffers = vec![MeshLayout::into_vertex_buffer_layout(
            0,
            D::format(),
            VertexStepMode::Vertex,
        )];

        let layout = vec![
            view_layout.clone(),
            model_layout.clone(),
            material_layout.as_ref().clone(),
        ];

        let id = cache.queue_render_pipeline(RenderPipelineDesc {
            label: None,
            layout,
            vertex: VertexState {
                shader: *vertex_shader.as_ref(),
                entry: "main".into(),
                buffers,
            },
            fragment: Some(FragmentState {
                shader: *fragment_shader.as_ref(),
                entry: "main".into(),
                targets: vec![Some(ColorTargetState {
                    format: surface.format(),
                    blend: Some(<D::Material as Material>::Phase::mode().into()),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: D::primitive(),
            depth_stencil: Some(wgpu::DepthStencilState {
                format: surface.depth_format(),
                depth_write_enabled: matches!(
                    <D::Material as Material>::depth_write(),
                    DepthWrite::Enabled
                ),
                depth_compare: wgpu::CompareFunction::Greater,
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            push_constants: vec![],
        });

        Ok(Self {
            pipeline: id,
            key: MeshKey::from(D::format()),
            _marker: Default::default(),
        })
    }
}

#[derive(Debug, Clone)]
pub enum DrawMode {
    Indirect {
        offset: u64,
    },
    IndexedIndirect {
        offset: u64,
        format: IndexFormat,
    },
    Direct {
        bind_group: usize,
        instances: Range<u32>,
    },
    IndexedDirect {
        bind_group: usize,
        instances: Range<u32>,
        format: IndexFormat,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum DrawError {
    #[error("Draw command skipped")]
    Skip,
}

pub type Draw<P> = fn(
    &mut RenderState<'_>,
    &ViewInstance<<P as RenderPhase>::View>,
    &DrawCall<P>,
    &World,
) -> Result<(), DrawError>;

pub struct DrawCall<P: RenderPhase> {
    pub material: ErasedId,
    pub mesh: AssetId<Mesh>,
    pub sub_mesh: SubMesh,
    pub mode: DrawMode,
    pub pipeline: PipelineId,
    pub item: P::Item,
    pub(super) function: Draw<P>,
}

impl<P: RenderPhase> DrawCall<P> {
    pub fn draw(
        &self,
        state: &mut RenderState<'_>,
        view: &ViewInstance<<P as RenderPhase>::View>,
        world: &World,
    ) -> Result<(), DrawError> {
        (self.function)(state, view, self, world)
    }
}

impl<P: RenderPhase> Clone for DrawCall<P> {
    fn clone(&self) -> Self {
        Self {
            material: self.material,
            mesh: self.mesh,
            sub_mesh: self.sub_mesh,
            mode: self.mode.clone(),
            pipeline: self.pipeline,
            item: self.item,
            function: self.function,
        }
    }
}

#[derive(Resource)]
pub struct ViewDrawSet<V: View, P: RenderPhase<View = V>>(
    pub(super) HashMap<Entity, Vec<DrawCall<P>>>,
);
impl<V: View, P: RenderPhase<View = V>> ViewDrawSet<V, P> {
    pub fn new() -> Self {
        Self(HashMap::new())
    }

    pub(super) fn draw<D>(
        state: &mut RenderState<'_>,
        view: &ViewInstance<P::View>,
        call: &DrawCall<P>,
        world: &World,
    ) -> Result<(), DrawError>
    where
        D: Drawable<View = V>,
        D::Material: Material<Phase = P>,
    {
        const VIEW: u32 = 0;
        const INSTANCES: u32 = 1;
        const MATERIAL: u32 = 2;

        let pipeline = world
            .resource::<PipelineCache>()
            .get_render_pipeline(&call.pipeline)
            .ok_or(DrawError::Skip)?;

        let views = world.resource::<ViewBuffer<V>>();
        let meshes = world.resource::<MeshAllocator>();
        let vertex = meshes.vertex_slice(&call.mesh).ok_or(DrawError::Skip)?;

        let materials = world.resource::<RenderAssets<MaterialInstance<D::Material>>>();
        let material = materials
            .get(&call.material.into())
            .ok_or(DrawError::Skip)?;

        state.set_pipeline(pipeline);
        state.set_vertex_buffer(0, vertex.buffer.slice(..));

        match &call.mode {
            DrawMode::Direct {
                bind_group,
                instances,
            } => {
                let models = world.resource::<UniformDataBuffer<D::Model>>();
                let bind_group = &models.bind_groups()[*bind_group];
                let vertices = call.sub_mesh.start_vertex + vertex.range.start
                    ..call.sub_mesh.start_vertex + call.sub_mesh.vertex_count + vertex.range.start;

                state.set_bind_group(VIEW, views.bind_group(), &[view.offset]);
                state.set_bind_group(INSTANCES, bind_group, &[]);
                state.set_bind_group(MATERIAL, &material, &[]);
                Ok(state.draw(vertices, instances.clone()))
            }
            DrawMode::IndexedDirect {
                bind_group,
                instances,
                format,
            } => {
                let index = meshes.index_slice(&call.mesh).ok_or(DrawError::Skip)?;
                let models = world.resource::<UniformDataBuffer<D::Model>>();
                let bind_group = &models.bind_groups()[*bind_group];
                let indices = call.sub_mesh.start_index + index.range.start
                    ..call.sub_mesh.start_index + call.sub_mesh.index_count + index.range.start;
                let base_vertex = (vertex.range.start + call.sub_mesh.start_vertex) as i32;

                state.set_index_buffer(index.buffer.slice(..), *format);
                state.set_bind_group(VIEW, views.bind_group(), &[view.offset]);
                state.set_bind_group(INSTANCES, bind_group, &[]);
                state.set_bind_group(MATERIAL, &material, &[]);
                Ok(state.draw_indexed(indices, base_vertex, instances.clone()))
            }
            DrawMode::Indirect { offset } => {
                let instances = world.resource::<StorageDataBuffer<D::Model>>();
                let draw_args = world.resource::<DrawArgsBuffer>();

                state.set_bind_group(VIEW, views.bind_group(), &[view.offset]);
                state.set_bind_group(INSTANCES, instances.bind_group(), &[]);
                state.set_bind_group(MATERIAL, &material, &[]);
                Ok(state.draw_indirect(draw_args.non_indexed().as_ref(), *offset))
            }
            DrawMode::IndexedIndirect { offset, format } => {
                let index = meshes.index_slice(&call.mesh).ok_or(DrawError::Skip)?;
                let instances = world.resource::<StorageDataBuffer<D::Model>>();
                let draw_args = world.resource::<DrawArgsBuffer>();

                state.set_index_buffer(index.buffer.slice(..), *format);
                state.set_bind_group(VIEW, views.bind_group(), &[view.offset]);
                state.set_bind_group(INSTANCES, instances.bind_group(), &[]);
                state.set_bind_group(MATERIAL, &material, &[]);
                Ok(state.draw_indexed_indirect(draw_args.indexed().as_ref(), *offset))
            }
        }
    }

    pub fn clear(&mut self) {
        self.0.clear();
    }

    pub fn clear_set(set: &mut Self) {
        set.clear();
    }
}
