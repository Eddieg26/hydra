use crate::{RenderAsset, device::RenderDevice, surface::RenderSurface};
use asset::{Asset, AssetId};
use ecs::unlifetime::Read;
use math::Size;
use std::sync::Arc;

#[derive(Clone, Copy, Debug, Asset, serde::Serialize, serde::Deserialize)]
pub struct RenderTexture {
    pub width: u32,
    pub height: u32,
}

impl RenderTexture {
    pub fn new(width: u32, height: u32) -> Self {
        Self { width, height }
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

#[derive(Asset)]
pub struct RenderTarget {
    size: Size<u32>,
    texture: Arc<wgpu::Texture>,
    view: wgpu::TextureView,
}

impl RenderTarget {
    pub fn size(&self) -> Size<u32> {
        self.size
    }

    pub fn width(&self) -> u32 {
        self.size.width
    }

    pub fn height(&self) -> u32 {
        self.size.height
    }

    pub fn texture(&self) -> &wgpu::Texture {
        &self.texture
    }

    pub fn view(&self) -> &wgpu::TextureView {
        &self.view
    }
}

impl RenderAsset for RenderTarget {
    type Source = RenderTexture;

    type Arg = (Read<RenderDevice>, Read<RenderSurface>);

    fn extract(
        _: AssetId<Self::Source>,
        texture: Self::Source,
        (device, surface): &mut ecs::ArgItem<Self::Arg>,
    ) -> Result<Self, crate::ExtractError<Self::Source>> {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width: texture.width,
                height: texture.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: surface.format(),
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        let view = texture.create_view(&Default::default());

        Ok(Self {
            size: Size::new(texture.width(), texture.height()),
            texture: Arc::new(texture),
            view,
        })
    }
}
