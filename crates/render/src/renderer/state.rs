use crate::{
    resources::{
        BindGroup, BindGroupId, Buffer, BufferSlice, BufferSliceId, IndexSlice, PipelineId,
        RenderPipeline,
    },
    types::{Color, Viewport},
};
use std::{collections::HashMap, ops::Range};
use wgpu::{IndexFormat, QuerySet, RenderBundle, ShaderStages};

pub struct RenderState<'a> {
    pass: wgpu::RenderPass<'a>,
    vertex_buffers: HashMap<u32, BufferSliceId>,
    index_buffer: Option<BufferSliceId>,
    bind_groups: HashMap<u32, (BindGroupId, Vec<u32>)>,
    pipeline: Option<PipelineId>,
}

impl<'a> RenderState<'a> {
    pub fn new(pass: wgpu::RenderPass<'a>) -> Self {
        Self {
            pass,
            vertex_buffers: HashMap::new(),
            index_buffer: None,
            bind_groups: HashMap::new(),
            pipeline: None,
        }
    }

    pub fn set_vertex_buffer(&mut self, slot: u32, slice: BufferSlice<'_>) {
        match self.vertex_buffers.get(&slot) {
            Some(id) if id != &slice.id() => {
                self.pass.set_vertex_buffer(slot, *slice);
                self.vertex_buffers.insert(slot, slice.id());
            }
            None => {
                self.pass.set_vertex_buffer(slot, *slice);
                self.vertex_buffers.insert(slot, slice.id());
            }
            _ => (),
        }
    }

    pub fn set_index_buffer(&mut self, slice: IndexSlice<'_>) {
        match self.index_buffer.as_ref() {
            Some(id) if id != &slice.id() => {
                self.pass.set_index_buffer(*slice, IndexFormat::Uint32);
                self.index_buffer = Some(slice.id());
            }
            None => {
                self.pass.set_index_buffer(*slice, IndexFormat::Uint32);
                self.index_buffer = Some(slice.id());
            }
            _ => (),
        }
    }

    pub fn set_bind_group(&mut self, group: u32, bind_group: &BindGroup, offsets: &[u32]) {
        match self.bind_groups.get(&group) {
            Some((id, bindings)) if id != &bind_group.id || bindings.as_slice() == offsets => {
                self.pass
                    .set_bind_group(group, Some(bind_group.as_ref()), offsets);
                self.bind_groups
                    .insert(group, (bind_group.id, offsets.to_vec()));
            }
            None => {
                self.pass
                    .set_bind_group(group, Some(bind_group.as_ref()), offsets);
                self.bind_groups
                    .insert(group, (bind_group.id, offsets.to_vec()));
            }
            _ => (),
        }
    }

    pub fn set_pipeline(&mut self, pipeline: &RenderPipeline) {
        if self.pipeline.as_ref() != Some(&pipeline.id()) {
            self.pass.set_pipeline(pipeline);
            self.pipeline = Some(pipeline.id());
        }
    }

    pub fn set_viewport(&mut self, viewport: Viewport) {
        let Viewport {
            x,
            y,
            width,
            height,
            depth,
        } = viewport;

        self.pass
            .set_viewport(x, y, width, height, depth.start, depth.end);
    }

    pub fn set_scissor_rect(&mut self, x: u32, y: u32, width: u32, height: u32) {
        self.pass.set_scissor_rect(x, y, width, height);
    }

    pub fn set_blend_constant(&mut self, color: Color) {
        self.pass.set_blend_constant(color.into());
    }

    pub fn set_push_constants(&mut self, stages: ShaderStages, offset: u32, data: &[u8]) {
        self.pass.set_push_constants(stages, offset, data);
    }

    pub fn set_stencil_reference(&mut self, reference: u32) {
        self.pass.set_stencil_reference(reference);
    }

    pub fn draw(&mut self, vertices: Range<u32>, instances: Range<u32>) {
        self.pass.draw(vertices, instances);
    }

    pub fn draw_indexed(&mut self, indices: Range<u32>, base_vertex: i32, instances: Range<u32>) {
        self.pass.draw_indexed(indices, base_vertex, instances);
    }

    pub fn draw_indirect(&mut self, buffer: &Buffer, offset: u64) {
        self.pass.draw_indirect(buffer.as_ref(), offset);
    }

    pub fn draw_indexed_indirect(&mut self, buffer: &Buffer, offset: u64) {
        self.pass.draw_indexed_indirect(buffer.as_ref(), offset);
    }

    pub fn multi_draw_indirect(&mut self, buffer: &Buffer, offset: u64, count: u32) {
        self.pass
            .multi_draw_indirect(buffer.as_ref(), offset, count);
    }

    pub fn multi_draw_indirect_count(
        &mut self,
        buffer: &Buffer,
        offset: u64,
        count_buffer: &Buffer,
        count_offset: u64,
        max_count: u32,
    ) {
        self.pass.multi_draw_indirect_count(
            buffer.as_ref(),
            offset,
            count_buffer.as_ref(),
            count_offset,
            max_count,
        );
    }

    pub fn multi_draw_indexed_indirect(&mut self, buffer: &Buffer, offset: u64, count: u32) {
        self.pass
            .multi_draw_indexed_indirect(buffer.as_ref(), offset, count);
    }

    pub fn multi_draw_indexed_indirect_count(
        &mut self,
        buffer: &Buffer,
        offset: u64,
        count_buffer: &Buffer,
        count_offset: u64,
        max_count: u32,
    ) {
        self.pass.multi_draw_indexed_indirect_count(
            buffer.as_ref(),
            offset,
            count_buffer.as_ref(),
            count_offset,
            max_count,
        );
    }

    pub fn push_debug_group(&mut self, label: &str) {
        self.pass.push_debug_group(label);
    }

    pub fn pop_debug_group(&mut self) {
        self.pass.pop_debug_group();
    }

    pub fn begin_occlusion_query(&mut self, query_index: u32) {
        self.pass.begin_occlusion_query(query_index);
    }

    pub fn end_occlusion_query(&mut self) {
        self.pass.end_occlusion_query();
    }

    pub fn begin_pipeline_statistics_query(&mut self, query_set: &QuerySet, query_index: u32) {
        self.pass
            .begin_pipeline_statistics_query(query_set, query_index);
    }

    pub fn end_pipeline_statistics_query(&mut self) {
        self.pass.end_pipeline_statistics_query();
    }

    pub fn execute_bundles<I>(&mut self, render_bundles: I)
    where
        I: IntoIterator<Item = &'a RenderBundle>,
    {
        self.pass.execute_bundles(render_bundles);
    }

    pub fn insert_debug_marker(&mut self, label: &str) {
        self.pass.insert_debug_marker(label);
    }

    pub fn write_timestamp(&mut self, query_set: &QuerySet, query_index: u32) {
        self.pass.write_timestamp(query_set, query_index);
    }

    pub fn clear(&mut self) {
        self.vertex_buffers.clear();
        self.index_buffer = None;
        self.bind_groups.clear();
        self.pipeline = None;
    }
}
