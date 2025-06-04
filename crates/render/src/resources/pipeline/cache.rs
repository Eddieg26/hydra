use super::{ComputePipeline, ComputePipelineDesc, PipelineId, RenderPipeline, RenderPipelineDesc};
use crate::{
    device::RenderDevice,
    resources::{ExtractInfo, Id, ShaderSource, extract::RenderAssets, shader::Shader},
};
use ecs::{IndexMap, IndexSet, Resource};
use std::collections::HashMap;

#[derive(Default, Clone, Debug, PartialEq)]
pub struct ShaderPipelines {
    pipelines: IndexSet<PipelineId>,
}

pub enum QueuedPipeline {
    Render {
        id: PipelineId,
        desc: RenderPipelineDesc,
    },
    Compute {
        id: PipelineId,
        desc: ComputePipelineDesc,
    },
}

#[derive(Default)]
pub struct PipelineCache {
    shaders: HashMap<Id<Shader>, ShaderPipelines>,
    render_pipelines: HashMap<PipelineId, RenderPipeline>,
    compute_pipelines: HashMap<PipelineId, ComputePipeline>,
    pipeline_queue: IndexMap<PipelineId, QueuedPipeline>,
}

impl PipelineCache {
    pub fn new() -> Self {
        Self {
            shaders: HashMap::new(),
            render_pipelines: HashMap::new(),
            compute_pipelines: HashMap::new(),
            pipeline_queue: IndexMap::new(),
        }
    }

    pub fn get_render_pipeline(&self, id: &PipelineId) -> Option<&RenderPipeline> {
        self.render_pipelines.get(id)
    }

    pub fn get_compute_pipeline(&self, id: &PipelineId) -> Option<&ComputePipeline> {
        self.compute_pipelines.get(id)
    }

    pub fn queue_render_pipeline(&mut self, desc: RenderPipelineDesc) -> PipelineId {
        let id = PipelineId::new();

        self.add_shader_dependency(&desc.vertex.shader, id);

        if let Some(fragment) = &desc.fragment {
            self.add_shader_dependency(&fragment.shader, id);
        }

        self.pipeline_queue
            .insert(id, QueuedPipeline::Render { id, desc });

        id
    }

    pub fn queue_compute_pipeline(&mut self, desc: ComputePipelineDesc) -> PipelineId {
        let id = PipelineId::new();

        self.add_shader_dependency(&desc.shader, id);

        self.pipeline_queue
            .insert(id, QueuedPipeline::Compute { id, desc });

        id
    }

    pub fn remove_render_pipeline(&mut self, id: PipelineId) -> Option<RenderPipeline> {
        let pipeline = self.render_pipelines.remove(&id);
        self.pipeline_queue.shift_remove(&id);

        pipeline
    }

    pub fn remove_compute_pipeline(&mut self, id: PipelineId) -> Option<ComputePipeline> {
        let pipeline = self.compute_pipelines.remove(&id);
        self.pipeline_queue.shift_remove(&id);

        pipeline
    }

    pub fn remove_shader(&mut self, shader: &Id<Shader>) {
        self.shaders.remove(shader);
    }

    pub fn process_queue(&mut self, device: &RenderDevice, shaders: &RenderAssets<Shader>) {
        let mut queue = IndexMap::new();

        for (_, pipeline) in self.pipeline_queue.drain(..) {
            match pipeline {
                QueuedPipeline::Render { id, desc } => {
                    match RenderPipeline::create(device, shaders, id, &desc) {
                        Some(pipeline) => {
                            self.render_pipelines.insert(id, pipeline);
                        }
                        None => {
                            queue.insert(id, QueuedPipeline::Render { id, desc });
                        }
                    }
                }
                QueuedPipeline::Compute { id, desc } => {
                    match ComputePipeline::create(device, shaders, id, &desc) {
                        Some(pipeline) => {
                            self.compute_pipelines.insert(id, pipeline);
                        }
                        None => {
                            queue.insert(id, QueuedPipeline::Compute { id, desc });
                        }
                    }
                }
            }
        }

        self.pipeline_queue = queue;
    }

    fn add_shader_dependency(&mut self, shader: &Id<Shader>, id: PipelineId) {
        self.shaders
            .entry(shader.clone())
            .or_default()
            .pipelines
            .insert(id);
    }

    pub(crate) fn process(
        pipelines: &mut PipelineCache,
        info: &ExtractInfo<ShaderSource>,
        device: &RenderDevice,
        shaders: &RenderAssets<Shader>,
    ) {
        pipelines.process_queue(&device, &shaders);
        for id in &info.removed {
            pipelines.remove_shader(id);
        }
    }
}

impl Resource for PipelineCache {}
