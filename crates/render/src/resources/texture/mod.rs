use super::{Label, RenderAssetExtractor, extract::RenderAsset};
use crate::device::RenderDevice;
use asset::Asset;
use ecs::system::unlifetime::Read;
use std::{ops::Range, sync::Arc};
use wgpu::{TextureAspect, TextureFormat};

pub mod fallbacks;
pub mod render;
pub mod sampler;

pub use fallbacks::*;
pub use render::*;
pub use sampler::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum TextureDimension {
    D1,
    D2,
    D2Array,
    D3,
    Cube,
    CubeArray,
}

impl Into<wgpu::TextureDimension> for TextureDimension {
    fn into(self) -> wgpu::TextureDimension {
        match self {
            TextureDimension::D1 => wgpu::TextureDimension::D1,
            TextureDimension::D2 => wgpu::TextureDimension::D2,
            TextureDimension::D3 => wgpu::TextureDimension::D3,
            TextureDimension::Cube => wgpu::TextureDimension::D2,
            TextureDimension::D2Array => wgpu::TextureDimension::D2,
            TextureDimension::CubeArray => wgpu::TextureDimension::D2,
        }
    }
}

impl Into<wgpu::TextureViewDimension> for TextureDimension {
    fn into(self) -> wgpu::TextureViewDimension {
        match self {
            TextureDimension::D1 => wgpu::TextureViewDimension::D1,
            TextureDimension::D2 => wgpu::TextureViewDimension::D2,
            TextureDimension::D3 => wgpu::TextureViewDimension::D3,
            TextureDimension::Cube => wgpu::TextureViewDimension::Cube,
            TextureDimension::D2Array => wgpu::TextureViewDimension::D2Array,
            TextureDimension::CubeArray => wgpu::TextureViewDimension::CubeArray,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum FilterMode {
    Nearest,
    Linear,
}

impl Into<wgpu::FilterMode> for FilterMode {
    fn into(self) -> wgpu::FilterMode {
        match self {
            FilterMode::Nearest => wgpu::FilterMode::Nearest,
            FilterMode::Linear => wgpu::FilterMode::Linear,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum WrapMode {
    Repeat,
    ClampToEdge,
    ClampToBorder,
    MirrorRepeat,
}

impl Into<wgpu::AddressMode> for WrapMode {
    fn into(self) -> wgpu::AddressMode {
        match self {
            WrapMode::Repeat => wgpu::AddressMode::Repeat,
            WrapMode::ClampToEdge => wgpu::AddressMode::ClampToEdge,
            WrapMode::ClampToBorder => wgpu::AddressMode::ClampToBorder,
            WrapMode::MirrorRepeat => wgpu::AddressMode::MirrorRepeat,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct TextureFace {
    pub start: usize,
    pub size: usize,
}

impl TextureFace {
    pub const fn new(start: usize, size: usize) -> Self {
        Self { start, size }
    }
}

#[derive(Clone, serde::Serialize, serde::Deserialize, Asset)]
pub struct Texture {
    pub label: Label,
    pub width: u32,
    pub height: u32,
    pub depth_or_layers: u32,
    pub mipmaps: bool,
    pub format: wgpu::TextureFormat,
    pub dimension: TextureDimension,
    pub filter: FilterMode,
    pub wrap: WrapMode,
    pub usage: wgpu::TextureUsages,
    pub pixels: Vec<u8>,
    pub layers: Vec<Range<usize>>,
}

impl Texture {
    pub fn new(
        size: wgpu::Extent3d,
        dimension: TextureDimension,
        format: wgpu::TextureFormat,
        pixels: Vec<u8>,
        layers: Vec<Range<usize>>,
    ) -> Self {
        Self {
            label: None,
            width: size.width,
            height: size.height,
            depth_or_layers: size.depth_or_array_layers,
            mipmaps: false,
            format,
            dimension,
            filter: FilterMode::Linear,
            wrap: WrapMode::ClampToBorder,
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::COPY_SRC,
            pixels,
            layers,
        }
    }

    pub fn default_white(dimension: TextureDimension) -> Self {
        match dimension {
            TextureDimension::D1 => Self::new(
                wgpu::Extent3d {
                    width: 1,
                    height: 1,
                    depth_or_array_layers: 1,
                },
                TextureDimension::D1,
                wgpu::TextureFormat::Rgba8Unorm,
                vec![255, 255, 255, 255],
                vec![0..4],
            ),
            TextureDimension::D2 => Self::new(
                wgpu::Extent3d {
                    width: 1,
                    height: 1,
                    depth_or_array_layers: 1,
                },
                TextureDimension::D2,
                wgpu::TextureFormat::Rgba8Unorm,
                vec![255, 255, 255, 255],
                vec![0..4],
            ),
            TextureDimension::D2Array => Self::new(
                wgpu::Extent3d {
                    width: 1,
                    height: 1,
                    depth_or_array_layers: 1,
                },
                TextureDimension::D2Array,
                wgpu::TextureFormat::Rgba8Unorm,
                vec![255, 255, 255, 255],
                vec![0..4],
            ),
            TextureDimension::D3 => Self::new(
                wgpu::Extent3d {
                    width: 1,
                    height: 1,
                    depth_or_array_layers: 1,
                },
                TextureDimension::D3,
                wgpu::TextureFormat::Rgba8Unorm,
                vec![255, 255, 255, 255],
                vec![0..4],
            ),
            TextureDimension::Cube => Self::new(
                wgpu::Extent3d {
                    width: 1,
                    height: 1,
                    depth_or_array_layers: 6,
                },
                TextureDimension::Cube,
                wgpu::TextureFormat::Rgba8Unorm,
                vec![[255u8, 255, 255, 255]; 6].concat(),
                (0..6).map(|i| i * 4..(i + 1) * 4).collect(),
            ),
            TextureDimension::CubeArray => Self::new(
                wgpu::Extent3d {
                    width: 1,
                    height: 1,
                    depth_or_array_layers: 6,
                },
                TextureDimension::CubeArray,
                wgpu::TextureFormat::Rgba8Unorm,
                vec![[255u8, 255, 255, 255]; 6].concat(),
                (0..6).map(|i| i * 4..(i + 1) * 4).collect(),
            ),
        }
    }

    pub fn with_label(mut self, label: Label) -> Self {
        self.label = label;
        self
    }

    pub fn with_mipmaps(mut self, mipmaps: bool) -> Self {
        self.mipmaps = mipmaps;
        self
    }

    pub fn with_filter(mut self, filter: FilterMode) -> Self {
        self.filter = filter;
        self
    }

    pub fn with_wrap(mut self, wrap: WrapMode) -> Self {
        self.wrap = wrap;
        self
    }

    pub fn with_usage(mut self, usage: wgpu::TextureUsages) -> Self {
        self.usage = usage;
        self
    }
}

pub struct GpuTexture {
    texture: Arc<wgpu::Texture>,
    view: wgpu::TextureView,
    sampler: Sampler,
    format: TextureFormat,
    width: u32,
    height: u32,
    mip_level_count: u32,
}

impl GpuTexture {
    pub fn new(
        texture: Arc<wgpu::Texture>,
        sampler: Sampler,
        format: TextureFormat,
        width: u32,
        height: u32,
        mip_level_count: u32,
    ) -> Self {
        let view = texture.create_view(&Default::default());

        Self {
            texture,
            view,
            sampler,
            format,
            width,
            height,
            mip_level_count,
        }
    }

    pub fn create(device: &RenderDevice, texture: &Texture, sampler: Sampler) -> Self {
        let size = wgpu::Extent3d {
            width: texture.width,
            height: texture.height,
            depth_or_array_layers: texture.depth_or_layers,
        };

        let mip_level_count = if texture.mipmaps {
            let dimension = texture.dimension.into();
            size.max_mips(dimension)
        } else {
            1
        };

        let format = texture.format;

        let created = device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width: texture.width,
                height: texture.height,
                depth_or_array_layers: texture.depth_or_layers,
            },
            mip_level_count,
            sample_count: 1,
            dimension: texture.dimension.into(),
            format,
            usage: texture.usage,
            view_formats: &[format],
        });

        let block_size = format.block_copy_size(None).unwrap_or(0);
        for (layer, range) in texture.layers.iter().enumerate() {
            device.queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &created,
                    mip_level: 0,
                    origin: wgpu::Origin3d {
                        x: 0,
                        y: 0,
                        z: layer as u32,
                    },
                    aspect: TextureAspect::All,
                },
                &texture.pixels[range.clone()],
                wgpu::TexelCopyBufferLayout {
                    bytes_per_row: Some(block_size * size.width),
                    rows_per_image: Some(block_size * size.width / size.height),
                    offset: 0,
                },
                wgpu::Extent3d {
                    width: size.width,
                    height: size.height,
                    depth_or_array_layers: 1,
                },
            );
        }

        let view = created.create_view(&wgpu::TextureViewDescriptor::default());

        Self {
            texture: Arc::new(created),
            view,
            sampler,
            mip_level_count,
            format,
            width: size.width,
            height: size.height,
        }
    }

    pub fn texture(&self) -> &wgpu::Texture {
        &self.texture
    }

    pub fn view(&self) -> &wgpu::TextureView {
        &self.view
    }

    pub fn sampler(&self) -> &Sampler {
        &self.sampler
    }

    pub fn format(&self) -> TextureFormat {
        self.format
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn mip_level_count(&self) -> u32 {
        self.mip_level_count
    }
}

impl std::ops::Deref for GpuTexture {
    type Target = wgpu::TextureView;

    fn deref(&self) -> &Self::Target {
        &self.view
    }
}

impl AsRef<wgpu::TextureView> for GpuTexture {
    fn as_ref(&self) -> &wgpu::TextureView {
        &self.view
    }
}

impl RenderAsset for GpuTexture {}

impl RenderAssetExtractor for Texture {
    type RenderAsset = GpuTexture;

    type Arg = Read<RenderDevice>;

    fn extract(
        texture: Self,
        device: &mut ecs::system::ArgItem<Self::Arg>,
    ) -> Result<Self::RenderAsset, super::ExtractError<Self>> {
        let sampler = Sampler::from_texture(device, &texture);
        Ok(GpuTexture::create(device, &texture, sampler))
    }
}
