use crate::{Texture, TextureDimension, plugins::RenderPlugin};
use asset::{AssetImporter, Settings, plugin::AssetAppExt};
use ecs::Plugin;

pub struct Texture2dPlugin;

impl Plugin for Texture2dPlugin {
    fn setup(&mut self, app: &mut ecs::AppBuilder) {
        app.add_plugins(RenderPlugin)
            .register_asset::<Texture>()
            .add_importer::<Texture2dImporter>();
    }
}

pub struct Texture2dImporter;

#[derive(
    Default, Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize,
)]
pub enum TextureImportFormat {
    #[default]
    RGBA8,
    RGBA16,
    RGBA32,
}

impl From<TextureImportFormat> for wgpu::TextureFormat {
    fn from(format: TextureImportFormat) -> Self {
        match format {
            TextureImportFormat::RGBA8 => wgpu::TextureFormat::Rgba8Unorm,
            TextureImportFormat::RGBA16 => wgpu::TextureFormat::Rgba16Unorm,
            TextureImportFormat::RGBA32 => wgpu::TextureFormat::Rgba32Float,
        }
    }
}

#[derive(Settings, Default, serde::Serialize, serde::Deserialize)]
pub struct Texture2dSettings {
    format: TextureImportFormat,
}

impl AssetImporter for Texture2dImporter {
    type Asset = Texture;

    type Settings = Texture2dSettings;

    type Error = image::ImageError;

    async fn import(
        ctx: &mut asset::importer::ImportContext<'_>,
        reader: &mut dyn asset::io::AsyncReader,
        metadata: &asset::AssetSettings<Self::Settings>,
    ) -> Result<Self::Asset, Self::Error> {
        use image::{error::*, *};

        let mut buffer = Vec::new();
        reader.read_to_end(&mut buffer).await.map_err(|e| {
            ImageError::Decoding(DecodingError::new(
                ImageFormatHint::PathExtension(ctx.path().to_path_buf()),
                e,
            ))
        })?;

        let img = ImageReader::new(std::io::Cursor::new(buffer))
            .with_guessed_format()?
            .decode()?;

        let width = img.width();
        let height = img.height();
        let pixels = match metadata.format {
            TextureImportFormat::RGBA8 => img.into_rgba8().into_raw(),
            TextureImportFormat::RGBA16 => {
                let rgba16 = img.into_rgba16().into_raw();
                bytemuck::cast_vec(rgba16)
            }
            TextureImportFormat::RGBA32 => {
                let rgba32 = img.into_rgba32f().into_raw();
                bytemuck::cast_vec(rgba32)
            }
        };

        let texture = Texture::new(
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            metadata.format.into(),
            pixels,
        );

        Ok(texture)
    }

    fn extensions() -> &'static [&'static str] {
        &[
            "png", "jpg", "jpeg", "gif", "webp", "tiff", "tga", "dds", "bmp", "ico", "hrd", "avif",
            "qoi",
        ]
    }
}
