use super::{
    AtomicId, Id,
    buffer::Buffer,
    texture::{GpuTexture, Sampler},
};
use crate::device::RenderDevice;
use ecs::system::{ArgItem, SystemArg};
use std::{error::Error, num::NonZero, sync::Arc};

#[derive(Clone, Debug, PartialEq)]
pub struct BindGroupLayout(Arc<wgpu::BindGroupLayout>);
impl From<wgpu::BindGroupLayout> for BindGroupLayout {
    fn from(layout: wgpu::BindGroupLayout) -> Self {
        Self(Arc::new(layout))
    }
}

impl std::ops::Deref for BindGroupLayout {
    type Target = wgpu::BindGroupLayout;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl AsRef<wgpu::BindGroupLayout> for BindGroupLayout {
    fn as_ref(&self) -> &wgpu::BindGroupLayout {
        &self.0
    }
}

pub type BindGroupId = AtomicId<BindGroup>;

#[derive(Clone, Debug, PartialEq)]
pub struct BindGroup {
    pub id: BindGroupId,
    bind_group: Arc<wgpu::BindGroup>,
}

impl From<wgpu::BindGroup> for BindGroup {
    fn from(group: wgpu::BindGroup) -> Self {
        Self {
            id: BindGroupId::new(),
            bind_group: Arc::new(group),
        }
    }
}

impl std::ops::Deref for BindGroup {
    type Target = wgpu::BindGroup;
    fn deref(&self) -> &Self::Target {
        &self.bind_group
    }
}

impl AsRef<wgpu::BindGroup> for BindGroup {
    fn as_ref(&self) -> &wgpu::BindGroup {
        &self.bind_group
    }
}

pub struct BindGroupLayoutBuilder {
    entries: Vec<wgpu::BindGroupLayoutEntry>,
}

impl BindGroupLayoutBuilder {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    pub fn with_buffer(
        &mut self,
        binding: u32,
        visibility: wgpu::ShaderStages,
        ty: wgpu::BufferBindingType,
        dynamic: bool,
        size: Option<wgpu::BufferSize>,
        count: Option<NonZero<u32>>,
    ) -> &mut Self {
        self.entries.push(wgpu::BindGroupLayoutEntry {
            binding,
            visibility,
            ty: wgpu::BindingType::Buffer {
                ty,
                has_dynamic_offset: dynamic,
                min_binding_size: size,
            },
            count,
        });
        self
    }

    pub fn with_uniform(
        &mut self,
        binding: u32,
        visibility: wgpu::ShaderStages,
        dynamic: bool,
        size: Option<wgpu::BufferSize>,
        count: Option<NonZero<u32>>,
    ) -> &mut Self {
        self.with_buffer(
            binding,
            visibility,
            wgpu::BufferBindingType::Uniform,
            dynamic,
            size,
            count,
        )
    }

    pub fn with_storage(
        &mut self,
        binding: u32,
        visibility: wgpu::ShaderStages,
        dynamic: bool,
        size: Option<wgpu::BufferSize>,
        count: Option<NonZero<u32>>,
    ) -> &mut Self {
        self.with_buffer(
            binding,
            visibility,
            wgpu::BufferBindingType::Storage { read_only: false },
            dynamic,
            size,
            count,
        )
    }

    pub fn with_texture(
        &mut self,
        binding: u32,
        visibility: wgpu::ShaderStages,
        dimension: wgpu::TextureViewDimension,
        sample_type: wgpu::TextureSampleType,
    ) -> &mut Self {
        self.entries.push(wgpu::BindGroupLayoutEntry {
            binding,
            visibility,
            ty: wgpu::BindingType::Texture {
                sample_type,
                view_dimension: dimension,
                multisampled: false,
            },
            count: None,
        });
        self
    }

    pub fn with_sampler(
        &mut self,
        binding: u32,
        visibility: wgpu::ShaderStages,
        ty: wgpu::SamplerBindingType,
    ) -> &mut Self {
        self.entries.push(wgpu::BindGroupLayoutEntry {
            binding,
            visibility,
            ty: wgpu::BindingType::Sampler(ty),
            count: None,
        });
        self
    }

    pub fn build(&self, device: &RenderDevice) -> BindGroupLayout {
        BindGroupLayout::from(
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &self.entries,
                label: None,
            }),
        )
    }
}

pub struct BindGroupBuilder<'a> {
    layout: &'a BindGroupLayout,
    entries: Vec<wgpu::BindGroupEntry<'a>>,
}

impl<'a> BindGroupBuilder<'a> {
    pub fn new(layout: &'a BindGroupLayout) -> Self {
        Self {
            layout,
            entries: Vec::new(),
        }
    }

    pub fn with_buffer(
        &mut self,
        binding: u32,
        buffer: &'a (impl AsRef<Buffer> + 'a),
        offset: wgpu::BufferAddress,
        size: Option<wgpu::BufferSize>,
    ) -> &mut Self {
        self.entries.push(wgpu::BindGroupEntry {
            binding,
            resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                buffer: buffer.as_ref().as_ref(),
                offset,
                size,
            }),
        });
        self
    }

    pub fn with_uniform(
        &mut self,
        binding: u32,
        buffer: &'a (impl AsRef<Buffer> + 'a),
        offset: wgpu::BufferAddress,
        size: Option<wgpu::BufferSize>,
    ) -> &mut Self {
        self.with_buffer(binding, buffer, offset, size)
    }

    pub fn with_storage(
        &mut self,
        binding: u32,
        buffer: &'a (impl AsRef<Buffer> + 'a),
        offset: wgpu::BufferAddress,
        size: Option<wgpu::BufferSize>,
    ) -> &mut Self {
        self.with_buffer(binding, buffer, offset, size)
    }

    pub fn with_texture(&mut self, binding: u32, view: &'a wgpu::TextureView) -> &mut Self {
        self.entries.push(wgpu::BindGroupEntry {
            binding,
            resource: wgpu::BindingResource::TextureView(view),
        });
        self
    }

    pub fn with_sampler(&mut self, binding: u32, sampler: &'a wgpu::Sampler) -> &mut Self {
        self.entries.push(wgpu::BindGroupEntry {
            binding,
            resource: wgpu::BindingResource::Sampler(sampler),
        });
        self
    }

    pub fn build(&self, device: &RenderDevice) -> BindGroup {
        BindGroup::from(device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: self.layout,
            entries: &self.entries,
            label: None,
        }))
    }
}

#[derive(Debug, Clone)]
pub enum CreateBindGroupError {
    Error(Arc<dyn Error + Send + Sync + 'static>),
    InvalidLayout,
    MissingTexture { id: Id<GpuTexture> },
    MissingSampler { id: Id<Sampler> },
    MissingBuffer,
}

impl CreateBindGroupError {
    pub fn from_error<E: Error + Send + Sync + 'static>(error: E) -> Self {
        Self::Error(Arc::new(error))
    }
}

impl std::fmt::Display for CreateBindGroupError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Error(error) => write!(f, "{}", error),
            Self::InvalidLayout => write!(f, "Invalid bind group layout"),
            Self::MissingTexture { id } => write!(f, "Missing texture: {:?}", id),
            Self::MissingSampler { id } => write!(f, "Missing sampler: {:?}", id),
            Self::MissingBuffer => write!(f, "Missing buffer"),
        }
    }
}

impl Error for CreateBindGroupError {}

pub trait AsBinding {
    type Arg: SystemArg;

    fn label() -> Option<&'static str> {
        None
    }

    fn create_bind_group(
        &self,
        device: &RenderDevice,
        layout: &BindGroupLayout,
        arg: &ArgItem<Self::Arg>,
    ) -> Result<BindGroup, CreateBindGroupError>;
    fn create_bind_group_layout(device: &RenderDevice) -> BindGroupLayout;
}
