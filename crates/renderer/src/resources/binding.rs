use crate::{
    core::RenderDevice,
    resources::{GpuResourceId, Label},
};
use std::{collections::HashMap, num::NonZero};
use wgpu::{
    BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayout, BindGroupLayoutDescriptor,
    BindGroupLayoutEntry, BindingResource, BindingType, BufferBinding, BufferBindingType, Sampler,
    SamplerBindingType, ShaderStages, TextureSampleType, TextureView, TextureViewDimension,
};

#[derive(Default, Clone)]
pub struct BindGroupLayoutBuilder {
    pub label: Option<Label>,
    pub entries: Vec<BindGroupLayoutEntry>,
}

impl BindGroupLayoutBuilder {
    pub fn new() -> Self {
        Self {
            label: None,
            entries: Vec::new(),
        }
    }

    pub fn with_label(&mut self, label: Label) -> &mut Self {
        self.label = Some(label);
        self
    }

    pub fn with_buffer(
        &mut self,
        ty: BufferBindingType,
        visibility: ShaderStages,
        has_dynamic_offset: bool,
        min_binding_size: Option<NonZero<u64>>,
        count: Option<NonZero<u32>>,
    ) -> &mut Self {
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
        &mut self,
        visibility: ShaderStages,
        has_dynamic_offset: bool,
        min_binding_size: Option<NonZero<u64>>,
        count: Option<NonZero<u32>>,
    ) -> &mut Self {
        self.with_buffer(
            BufferBindingType::Uniform,
            visibility,
            has_dynamic_offset,
            min_binding_size,
            count,
        )
    }

    pub fn with_storage(
        &mut self,
        read_only: bool,
        visibility: ShaderStages,
        has_dynamic_offset: bool,
        min_binding_size: Option<NonZero<u64>>,
        count: Option<NonZero<u32>>,
    ) -> &mut Self {
        self.with_buffer(
            BufferBindingType::Storage { read_only },
            visibility,
            has_dynamic_offset,
            min_binding_size,
            count,
        )
    }

    pub fn with_texture(
        &mut self,
        visibility: ShaderStages,
        sample_type: TextureSampleType,
        view_dimension: TextureViewDimension,
        multisampled: bool,
        count: Option<NonZero<u32>>,
    ) -> &mut Self {
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
        &mut self,
        visibility: ShaderStages,
        ty: SamplerBindingType,
        count: Option<NonZero<u32>>,
    ) -> &mut Self {
        let binding = self.entries.len() as u32;
        self.entries.push(BindGroupLayoutEntry {
            binding,
            visibility,
            ty: BindingType::Sampler(ty),
            count,
        });
        self
    }
}

impl From<Vec<BindGroupLayoutEntry>> for BindGroupLayoutBuilder {
    fn from(value: Vec<BindGroupLayoutEntry>) -> Self {
        Self {
            label: None,
            entries: value,
        }
    }
}

pub struct BindGroupBuilder<'a> {
    label: Option<Label>,
    entries: Vec<BindGroupEntry<'a>>,
}

impl<'a> BindGroupBuilder<'a> {
    pub fn new() -> Self {
        Self {
            label: None,
            entries: Vec::new(),
        }
    }

    pub fn with_label(&mut self, label: Label) -> &mut Self {
        self.label = Some(label);
        self
    }

    pub fn with_buffer(&mut self, buffer: BufferBinding<'a>) -> &mut Self {
        let binding = self.entries.len() as u32;
        self.entries.push(BindGroupEntry {
            binding,
            resource: BindingResource::Buffer(buffer),
        });

        self
    }

    pub fn with_buffer_array(&mut self, buffers: &'a [BufferBinding<'_>]) -> &mut Self {
        let binding = self.entries.len() as u32;
        self.entries.push(BindGroupEntry {
            binding,
            resource: BindingResource::BufferArray(buffers),
        });
        self
    }

    pub fn with_texture(&mut self, texture: &'a TextureView) -> &mut Self {
        let binding = self.entries.len() as u32;
        self.entries.push(BindGroupEntry {
            binding,
            resource: BindingResource::TextureView(texture),
        });
        self
    }

    pub fn with_texture_array(&mut self, textures: &'a [&TextureView]) -> &mut Self {
        let binding = self.entries.len() as u32;
        self.entries.push(BindGroupEntry {
            binding,
            resource: BindingResource::TextureViewArray(textures),
        });
        self
    }

    pub fn with_sampler(&mut self, sampler: &'a Sampler) -> &mut Self {
        let binding = self.entries.len() as u32;
        self.entries.push(BindGroupEntry {
            binding,
            resource: BindingResource::Sampler(sampler),
        });
        self
    }

    pub fn with_sampler_array(&mut self, samplers: &'a [&Sampler]) -> &mut Self {
        let binding = self.entries.len() as u32;
        self.entries.push(BindGroupEntry {
            binding,
            resource: BindingResource::SamplerArray(samplers),
        });
        self
    }

    pub fn build(self, device: &RenderDevice, layout: &'a BindGroupLayout) -> BindGroup {
        device.create_bind_group(&BindGroupDescriptor {
            label: self.label.as_deref(),
            layout,
            entries: &self.entries,
        })
    }
}

pub struct BindGroupLayoutRegistry {
    layouts: Vec<BindGroupLayout>,
    map: HashMap<Vec<BindGroupLayoutEntry>, GpuResourceId<BindGroupLayout>>,
}

impl BindGroupLayoutRegistry {
    pub fn new() -> Self {
        Self {
            layouts: Vec::new(),
            map: HashMap::new(),
        }
    }

    pub fn get(&self, id: GpuResourceId<BindGroupLayout>) -> &BindGroupLayout {
        &self.layouts[id.get() as usize]
    }

    pub fn id(&self, key: &[BindGroupLayoutEntry]) -> Option<GpuResourceId<BindGroupLayout>> {
        self.map.get(key).copied()
    }

    pub fn register(
        &mut self,
        device: &RenderDevice,
        builder: BindGroupLayoutBuilder,
    ) -> GpuResourceId<BindGroupLayout> {
        if let Some(id) = self.id(&builder.entries) {
            id
        } else {
            let id = GpuResourceId::new(self.layouts.len() as u32);
            let layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
                label: builder.label.as_deref(),
                entries: &builder.entries,
            });

            self.layouts.push(layout);
            self.map.insert(builder.entries, id);

            id
        }
    }
}
