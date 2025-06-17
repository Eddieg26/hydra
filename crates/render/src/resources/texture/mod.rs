use super::{Label, RenderAssetExtractor, extract::RenderAsset};
use crate::device::RenderDevice;
use asset::{Asset, Settings, importer::AssetImporter};
use ecs::system::unlifetime::Read;
use smol::io::AsyncAsSync;
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
            filter: FilterMode::Nearest,
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
                    rows_per_image: Some(size.height),
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

pub struct Texture2dImporter;

#[derive(
    Default, Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize,
)]
pub enum ImportFormat {
    #[default]
    RGBA8,
    RGBA16,
    RGBA32,
}

impl From<ImportFormat> for wgpu::TextureFormat {
    fn from(format: ImportFormat) -> Self {
        match format {
            ImportFormat::RGBA8 => wgpu::TextureFormat::Rgba8Unorm,
            ImportFormat::RGBA16 => wgpu::TextureFormat::Rgba16Unorm,
            ImportFormat::RGBA32 => wgpu::TextureFormat::Rgba32Float,
        }
    }
}

#[derive(Settings, Default, serde::Serialize, serde::Deserialize)]
pub struct Texture2dSettings {
    format: ImportFormat,
}

impl AssetImporter for Texture2dImporter {
    type Asset = Texture;

    type Settings = Texture2dSettings;

    type Error = image::ImageError;

    async fn import(
        ctx: &mut asset::importer::ImportContext<'_>,
        reader: &mut dyn asset::io::AsyncReader,
        metadata: &asset::AssetMetadata<Self::Settings>,
    ) -> Result<Self::Asset, Self::Error> {
        use image::error::*;
        let format = match ctx.path().ext() {
            Some("png") => image::ImageFormat::Png,
            Some("jpg") | Some("jpeg") => image::ImageFormat::Jpeg,
            Some(ext) => {
                return Err(ImageError::Unsupported(
                    UnsupportedError::from_format_and_kind(
                        ImageFormatHint::Unknown,
                        UnsupportedErrorKind::Format(ImageFormatHint::Name(ext.to_string())),
                    ),
                ));
            }
            None => {
                return Err(ImageError::Unsupported(
                    UnsupportedError::from_format_and_kind(
                        ImageFormatHint::Unknown,
                        UnsupportedErrorKind::Format(ImageFormatHint::Unknown),
                    ),
                ));
            }
        };

        let waker = waker_fn::waker_fn(|| {});
        let mut context = std::task::Context::from_waker(&waker);
        let reader = std::io::BufReader::new(AsyncAsSync::new(&mut context, reader));

        let dynamic = image::load(reader, format)?;
        let width = dynamic.width();
        let height = dynamic.height();
        let pixels = match metadata.format {
            ImportFormat::RGBA8 => dynamic.into_rgba8().into_raw(),
            ImportFormat::RGBA16 => {
                let rgba16 = dynamic.into_rgba16().into_raw();
                bytemuck::cast_vec(rgba16)
            }
            ImportFormat::RGBA32 => {
                let rgba32 = dynamic.into_rgba32f().into_raw();
                bytemuck::cast_vec(rgba32)
            }
        };

        let pixel_count = pixels.len();

        let texture = Texture::new(
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            metadata.format.into(),
            pixels,
            vec![0..pixel_count],
        );

        Ok(texture)
    }

    fn extensions() -> &'static [&'static str] {
        &["png", "jpg", "jpeg"]
    }
}
