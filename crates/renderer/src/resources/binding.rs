use crate::{core::RenderDevice, resources::Label};
use std::num::NonZero;
use wgpu::{
    BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayout, BindGroupLayoutDescriptor,
    BindGroupLayoutEntry, BindingResource, BindingType, BufferBinding, BufferBindingType, Sampler,
    SamplerBindingType, ShaderStages, TextureSampleType, TextureView, TextureViewDimension,
};

pub struct BindGroupLayoutBuilder {
    label: Option<Label>,
    entries: Vec<BindGroupLayoutEntry>,
}

impl BindGroupLayoutBuilder {
    pub fn new() -> Self {
        Self {
            label: None,
            entries: Vec::new(),
        }
    }

    pub fn with_label(mut self, label: Label) -> Self {
        self.label = Some(label);
        self
    }

    pub fn with_buffer(
        mut self,
        ty: BufferBindingType,
        visibility: ShaderStages,
        has_dynamic_offset: bool,
        min_binding_size: Option<NonZero<u64>>,
        count: Option<NonZero<u32>>,
    ) -> Self {
        let binding = self.entries.len() as u32;
        self.entries.push(BindGroupLayoutEntry {
            binding,
            visibility,
            ty: BindingType::Buffer {
                ty,
                has_dynamic_offset,
                min_binding_size,
            },
            count,
        });

        self
    }

    pub fn with_uniform(
        self,
        visibility: ShaderStages,
        has_dynamic_offset: bool,
        min_binding_size: Option<NonZero<u64>>,
        count: Option<NonZero<u32>>,
    ) -> Self {
        self.with_buffer(
            BufferBindingType::Uniform,
            visibility,
            has_dynamic_offset,
            min_binding_size,
            count,
        )
    }

    pub fn with_storage(
        self,
        read_only: bool,
        visibility: ShaderStages,
        has_dynamic_offset: bool,
        min_binding_size: Option<NonZero<u64>>,
        count: Option<NonZero<u32>>,
    ) -> Self {
        self.with_buffer(
            BufferBindingType::Storage { read_only },
            visibility,
            has_dynamic_offset,
            min_binding_size,
            count,
        )
    }

    pub fn with_texture(
        mut self,
        visibility: ShaderStages,
        sample_type: TextureSampleType,
        view_dimension: TextureViewDimension,
        multisampled: bool,
        count: Option<NonZero<u32>>,
    ) -> Self {
        let binding = self.entries.len() as u32;
        self.entries.push(BindGroupLayoutEntry {
            binding,
            visibility,
            ty: BindingType::Texture {
                sample_type,
                view_dimension,
                multisampled,
            },
            count,
        });
        self
    }

    pub fn with_sampler(
        mut self,
        visibility: ShaderStages,
        ty: SamplerBindingType,
        count: Option<NonZero<u32>>,
    ) -> Self {
        let binding = self.entries.len() as u32;
        self.entries.push(BindGroupLayoutEntry {
            binding,
            visibility,
            ty: BindingType::Sampler(ty),
            count,
        });
        self
    }

    pub fn build(self, device: &RenderDevice) -> BindGroupLayout {
        device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: self.label.as_deref(),
            entries: &self.entries,
        })
    }
}

pub struct BindGroupBuilder<'a> {
    label: Option<Label>,
    layout: &'a BindGroupLayout,
    entries: Vec<BindGroupEntry<'a>>,
}

impl<'a> BindGroupBuilder<'a> {
    pub fn new(layout: &'a BindGroupLayout) -> Self {
        Self {
            label: None,
            layout,
            entries: Vec::new(),
        }
    }

    pub fn with_label(mut self, label: Label) -> Self {
        self.label = Some(label);
        self
    }

    pub fn with_buffer(mut self, buffer: BufferBinding<'a>) -> Self {
        let binding = self.entries.len() as u32;
        self.entries.push(BindGroupEntry {
            binding,
            resource: BindingResource::Buffer(buffer),
        });

        self
    }

    pub fn with_buffer_array(mut self, buffers: &'a [BufferBinding<'_>]) -> Self {
        let binding = self.entries.len() as u32;
        self.entries.push(BindGroupEntry {
            binding,
            resource: BindingResource::BufferArray(buffers),
        });
        self
    }

    pub fn with_texture(mut self, texture: &'a TextureView) -> Self {
        let binding = self.entries.len() as u32;
        self.entries.push(BindGroupEntry {
            binding,
            resource: BindingResource::TextureView(texture),
        });
        self
    }

    pub fn with_texture_array(mut self, textures: &'a [&TextureView]) -> Self {
        let binding = self.entries.len() as u32;
        self.entries.push(BindGroupEntry {
            binding,
            resource: BindingResource::TextureViewArray(textures),
        });
        self
    }

    pub fn with_sampler(mut self, sampler: &'a Sampler) -> Self {
        let binding = self.entries.len() as u32;
        self.entries.push(BindGroupEntry {
            binding,
            resource: BindingResource::Sampler(sampler),
        });
        self
    }

    pub fn with_sampler_array(mut self, samplers: &'a [&Sampler]) -> Self {
        let binding = self.entries.len() as u32;
        self.entries.push(BindGroupEntry {
            binding,
            resource: BindingResource::SamplerArray(samplers),
        });
        self
    }

    pub fn build(self, device: &RenderDevice) -> BindGroup {
        device.create_bind_group(&BindGroupDescriptor {
            label: self.label.as_deref(),
            layout: self.layout,
            entries: &self.entries,
        })
    }
}
