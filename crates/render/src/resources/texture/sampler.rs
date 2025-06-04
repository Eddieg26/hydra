use super::{FilterMode, Texture, WrapMode};
use crate::{device::RenderDevice, resources::Label};
use std::sync::Arc;
use wgpu::{CompareFunction, SamplerBorderColor};

#[derive(Debug, Clone, PartialEq)]
pub struct SamplerDesc {
    pub label: Label,
    pub wrap_mode: WrapMode,
    pub filter_mode: FilterMode,
    pub lod_min_clamp: f32,
    pub lod_max_clamp: f32,
    pub compare: Option<CompareFunction>,
    pub anisotropy_clamp: u16,
    pub border_color: Option<SamplerBorderColor>,
}

impl Default for SamplerDesc {
    fn default() -> Self {
        Self {
            label: None,
            wrap_mode: WrapMode::ClampToEdge,
            filter_mode: FilterMode::Linear,
            lod_min_clamp: 0.0,
            lod_max_clamp: 100.0,
            compare: None,
            anisotropy_clamp: 1,
            border_color: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Sampler(Arc<wgpu::Sampler>);

impl Sampler {
    pub fn new(device: &RenderDevice, desc: &SamplerDesc) -> Self {
        let address_mode = desc.wrap_mode.into();
        let filter_mode = desc.filter_mode.into();

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: None,
            address_mode_u: address_mode,
            address_mode_v: address_mode,
            address_mode_w: address_mode,
            mag_filter: filter_mode,
            min_filter: filter_mode,
            mipmap_filter: filter_mode,
            lod_min_clamp: desc.lod_min_clamp,
            lod_max_clamp: desc.lod_max_clamp,
            compare: desc.compare,
            anisotropy_clamp: desc.anisotropy_clamp,
            border_color: desc.border_color,
        });

        Self(Arc::new(sampler))
    }

    pub fn from_texture(device: &RenderDevice, texture: &Texture) -> Self {
        Self::new(
            device,
            &SamplerDesc {
                label: None,
                wrap_mode: texture.wrap,
                filter_mode: texture.filter,
                border_color: match texture.wrap {
                    WrapMode::ClampToBorder => Some(wgpu::SamplerBorderColor::TransparentBlack),
                    _ => None,
                },
                ..Default::default()
            },
        )
    }

    pub fn inner(&self) -> &wgpu::Sampler {
        &self.0
    }
}

impl std::ops::Deref for Sampler {
    type Target = wgpu::Sampler;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl AsRef<wgpu::Sampler> for Sampler {
    fn as_ref(&self) -> &wgpu::Sampler {
        &self.0
    }
}

impl From<wgpu::Sampler> for Sampler {
    fn from(sampler: wgpu::Sampler) -> Self {
        Self(Arc::new(sampler))
    }
}
