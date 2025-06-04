use crate::{device::RenderDevice, resources::{ExtractError, RenderAssetExtractor}, surface::RenderSurface};
use asset::Asset;
use ecs::system::unlifetime::Read;
use std::sync::Arc;

use super::{FilterMode, GpuTexture, Sampler, SamplerDesc, WrapMode};

#[derive(Clone, Copy, Debug, Asset, serde::Serialize, serde::Deserialize)]
pub struct RenderTexture {
    pub width: u32,
    pub height: u32,
    pub wrap: WrapMode,
    pub filter: FilterMode,
}

impl RenderTexture {
    pub fn new(width: u32, height: u32, wrap: WrapMode, filter: FilterMode) -> Self {
        Self {
            width,
            height,
            wrap,
            filter,
        }
    }
}

impl RenderTexture {
    pub fn create_texture(&self, device: &RenderDevice, surface: &RenderSurface) -> wgpu::Texture {
        device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Render Texture"),
            size: wgpu::Extent3d {
                width: self.width,
                height: self.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: surface.format(),
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        })
    }
}

impl RenderAssetExtractor for RenderTexture {
    type RenderAsset = GpuTexture;

    type Arg = (Read<RenderDevice>, Read<RenderSurface>);

    fn extract(
        asset: Self,
        arg: &mut ecs::prelude::ArgItem<Self::Arg>,
    ) -> Result<Self::RenderAsset, ExtractError<Self>> {
        let (device, surface) = arg;

        let texture = asset.create_texture(device, surface);
        let sampler = Sampler::new(
            &device,
            &SamplerDesc {
                label: None,
                wrap_mode: asset.wrap,
                filter_mode: asset.filter,
                border_color: match asset.wrap {
                    WrapMode::ClampToBorder => Some(wgpu::SamplerBorderColor::TransparentBlack),
                    _ => None,
                },
                ..Default::default()
            },
        );

        let texture = GpuTexture::new(
            Arc::new(texture),
            sampler,
            surface.format(),
            asset.width,
            asset.height,
            1,
        );

        Ok(texture)
    }
}
