use super::{AtomicId, Label, binding::BindGroupLayout, extract::RenderAssets, shader::GpuShader};
use crate::device::RenderDevice;
use asset::AssetId;
use std::{borrow::Cow, sync::Arc};
use wgpu::{
    BufferAddress, ColorTargetState, DepthStencilState, MultisampleState, PrimitiveState,
    PushConstantRange, VertexAttribute, VertexStepMode,
};

pub mod cache;

pub use cache::*;

#[derive(Default, Clone, Debug, Hash, Eq, PartialEq)]
pub struct VertexBufferLayout {
    /// The stride, in bytes, between elements of this buffer.
    pub array_stride: BufferAddress,
    /// How often this vertex buffer is "stepped" forward.
    pub step_mode: VertexStepMode,
    /// The list of attributes which comprise a single vertex.
    pub attributes: Vec<VertexAttribute>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct VertexState {
    pub shader: AssetId<GpuShader>,
    pub entry: Cow<'static, str>,
    pub buffers: Vec<VertexBufferLayout>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FragmentState {
    pub shader: AssetId<GpuShader>,
    pub entry: Cow<'static, str>,
    pub targets: Vec<Option<ColorTargetState>>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RenderPipelineDesc {
    pub label: Label,
    pub layout: Vec<BindGroupLayout>,
    pub vertex: VertexState,
    pub fragment: Option<FragmentState>,
    pub primitive: PrimitiveState,
    pub depth_stencil: Option<DepthStencilState>,
    pub multisample: MultisampleState,
    pub push_constants: Vec<PushConstantRange>,
}

#[derive(Default, Clone, Debug, Hash, Eq, PartialEq)]
pub struct Pipeline;

pub type PipelineId = AtomicId<Pipeline>;

pub struct RenderPipeline {
    id: PipelineId,
    pipeline: Arc<wgpu::RenderPipeline>,
}

impl RenderPipeline {
    pub fn new(id: PipelineId, pipeline: wgpu::RenderPipeline) -> Self {
        Self {
            id,
            pipeline: Arc::new(pipeline),
        }
    }

    pub fn create(
        device: &RenderDevice,
        shaders: &RenderAssets<GpuShader>,
        id: PipelineId,
        desc: &RenderPipelineDesc,
    ) -> Option<Self> {
        let vertex_shader = shaders.get(desc.vertex.shader.as_ref())?;

        let fragment_shader = match &desc.fragment {
            Some(fragment) => Some(shaders.get(fragment.shader.as_ref())?),
            None => None,
        };

        let bind_group_layouts = desc
            .layout
            .iter()
            .map(|layout| layout.as_ref())
            .collect::<Vec<_>>();

        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: desc.label.as_deref(),
            bind_group_layouts: &bind_group_layouts,
            push_constant_ranges: &desc.push_constants,
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: desc.label.as_deref(),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: vertex_shader,
                entry_point: Some(&desc.vertex.entry),
                buffers: &desc
                    .vertex
                    .buffers
                    .iter()
                    .map(|buffer| wgpu::VertexBufferLayout {
                        array_stride: buffer.array_stride,
                        step_mode: buffer.step_mode,
                        attributes: &buffer.attributes,
                    })
                    .collect::<Vec<_>>(),
                compilation_options: Default::default(),
            },
            fragment: desc.fragment.as_ref().map(|fragment| wgpu::FragmentState {
                module: fragment_shader.unwrap(),
                entry_point: Some(&fragment.entry),
                targets: &fragment.targets,
                compilation_options: Default::default(),
            }),
            primitive: desc.primitive,
            depth_stencil: desc.depth_stencil.clone(),
            multisample: desc.multisample,
            cache: None,
            multiview: None,
        });

        Some(Self::new(id, pipeline))
    }

    pub fn id(&self) -> PipelineId {
        self.id
    }
}

impl From<wgpu::RenderPipeline> for RenderPipeline {
    fn from(pipeline: wgpu::RenderPipeline) -> Self {
        Self {
            id: PipelineId::new(),
            pipeline: Arc::new(pipeline),
        }
    }
}

impl std::ops::Deref for RenderPipeline {
    type Target = wgpu::RenderPipeline;
    fn deref(&self) -> &Self::Target {
        &self.pipeline
    }
}

impl AsRef<wgpu::RenderPipeline> for RenderPipeline {
    fn as_ref(&self) -> &wgpu::RenderPipeline {
        &self.pipeline
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ComputePipelineDesc {
    pub label: Label,
    pub layout: Vec<BindGroupLayout>,
    pub shader: AssetId<GpuShader>,
    pub entry: Cow<'static, str>,
}

pub struct ComputePipeline {
    id: PipelineId,
    pipeline: Arc<wgpu::ComputePipeline>,
}

impl ComputePipeline {
    pub fn new(id: PipelineId, pipeline: wgpu::ComputePipeline) -> Self {
        Self {
            id,
            pipeline: Arc::new(pipeline),
        }
    }

    pub fn create(
        device: &RenderDevice,
        shaders: &RenderAssets<GpuShader>,
        id: PipelineId,
        desc: &ComputePipelineDesc,
    ) -> Option<Self> {
        let shader = shaders.get(desc.shader.as_ref())?;

        let bind_group_layouts = desc
            .layout
            .iter()
            .map(|layout| layout.as_ref())
            .collect::<Vec<_>>();

        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: desc.label.as_deref(),
            bind_group_layouts: &bind_group_layouts,
            push_constant_ranges: &[],
        });

        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: desc.label.as_deref(),
            layout: Some(&layout),
            module: shader,
            entry_point: Some(&desc.entry),
            compilation_options: Default::default(),
            cache: None,
        });

        Some(Self::new(id, pipeline))
    }

    pub fn id(&self) -> PipelineId {
        self.id
    }
}

impl From<wgpu::ComputePipeline> for ComputePipeline {
    fn from(pipeline: wgpu::ComputePipeline) -> Self {
        Self {
            id: PipelineId::new(),
            pipeline: Arc::new(pipeline),
        }
    }
}

impl std::ops::Deref for ComputePipeline {
    type Target = wgpu::ComputePipeline;
    fn deref(&self) -> &Self::Target {
        &self.pipeline
    }
}

impl AsRef<wgpu::ComputePipeline> for ComputePipeline {
    fn as_ref(&self) -> &wgpu::ComputePipeline {
        &self.pipeline
    }
}
