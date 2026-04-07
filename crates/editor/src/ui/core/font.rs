use asset::{Asset, AssetId};
use fontdue::LineMetrics;
use math::{Size, Vec2, rect::Rect};
use renderer::{
    core::RenderDevice,
    resources::{ExtractError, GpuTexture, RenderAsset, RenderAssets, SamplerCache, Texture},
};
use std::collections::{HashMap, HashSet};

#[derive(Clone, Asset)]
pub struct Font {
    data: Vec<u8>,
}

pub struct GpuFont {
    inner: fontdue::Font,
    metrics: HashMap<u32, LineMetrics>,
    kern: HashMap<(char, char), f32>,
    glyphs: HashMap<GlyphEntry, Glyph>,
}

impl GpuFont {
    pub fn metrics(&self, px: f32) -> Option<&LineMetrics> {
        self.metrics.get(&px.to_bits())
    }

    pub fn add_metrics(&mut self, px: f32) -> Option<LineMetrics> {
        let metrics = self.inner.horizontal_line_metrics(px)?;
        self.metrics.insert(px.to_bits(), metrics);
        Some(metrics)
    }

    pub fn kern(&mut self, left: char, right: char, px: u32) -> Option<f32> {
        if let Some(value) = self.kern.get(&(left, right)).copied() {
            Some(value)
        } else {
            let value = self
                .inner
                .horizontal_kern(left, right, f32::from_bits(px))?;
            self.kern.insert((left, right), value);
            Some(value)
        }
    }

    pub fn glyph(&self, ch: char, size: u32) -> Option<&Glyph> {
        self.glyphs.get(&GlyphEntry { ch, size })
    }
}

impl RenderAsset for GpuFont {
    type Asset = Font;

    type Arg = ();

    fn extract(
        _: AssetId<Self::Asset>,
        asset: Self::Asset,
        _: &mut ecs::ArgItem<Self::Arg>,
    ) -> Result<Self, renderer::resources::ExtractError<Self::Asset>> {
        let inner = fontdue::Font::from_bytes(asset.data, fontdue::FontSettings::default())
            .map_err(|e| ExtractError::Custom(e.to_owned()))?;

        Ok(GpuFont {
            inner,
            metrics: HashMap::new(),
            kern: HashMap::new(),
            glyphs: HashMap::new(),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GlyphKey {
    pub font: AssetId<Font>,
    pub ch: char,
    pub size: u32,
}

impl GlyphKey {
    pub fn new(font: AssetId<Font>, ch: char, size: u32) -> Self {
        Self { font, ch, size }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GlyphEntry {
    pub ch: char,
    pub size: u32,
}

#[derive(Debug, Clone, Copy)]
pub struct Glyph {
    pub rect: Rect,
    pub uv: Rect,
    pub advance: Vec2,
}

#[derive(Default)]
pub struct GlyphQueue(HashMap<AssetId<Font>, HashSet<GlyphEntry>>);

impl GlyphQueue {
    pub fn queue(&mut self, key: GlyphKey) {
        self.0.entry(key.font).or_default().insert(GlyphEntry {
            ch: key.ch,
            size: key.size,
        });
    }
}

pub struct GlyphAtlas {
    cursor: Vec2,
    size: Size,
    row_height: f32,
    padding: f32,
    glyphs: HashMap<GlyphKey, Glyph>,
}

impl GlyphAtlas {
    pub const ID: AssetId<Texture> = AssetId::from_u128(0);

    pub fn update(
        &mut self,
        device: &RenderDevice,
        fonts: &mut RenderAssets<GpuFont>,
        queue: &mut GlyphQueue,
        textures: &mut RenderAssets<GpuTexture>,
    ) {
        let atlas = match textures.get_mut(&GlyphAtlas::ID) {
            Some(atlas) => atlas,
            None => {
                textures.insert(GlyphAtlas::ID, self.create_atlas(device));
                textures.get_mut(&GlyphAtlas::ID).unwrap()
            }
        };

        for (id, entries) in queue.0.drain() {
            let Some(font) = fonts.get_mut(&id) else {
                continue;
            };

            for entry in entries {
                let (metrics, bitmap) = font.inner.rasterize(entry.ch, entry.size as f32);
                let glyph_w = metrics.width as f32;
                let glyph_h = metrics.height as f32;
                let padding = self.padding.ceil();
                let padded_w = (glyph_w + padding * 2.0).ceil();
                let padded_h = (glyph_h + padding * 2.0).ceil();

                let Some(Vec2 { x, y }) = self.advance(padded_w, padded_h) else {
                    continue;
                };

                let rect = Rect::new(metrics.xmin as f32, metrics.ymin as f32, glyph_w, glyph_h);
                let uv = Rect::new(
                    (x + padding) / self.size.width,
                    (y + padding) / self.size.height,
                    glyph_w / self.size.width,
                    glyph_h / self.size.height,
                );

                let advance = Vec2::new(metrics.advance_width, metrics.advance_height);
                let key = GlyphKey::new(id, entry.ch, entry.size);
                let glyph = Glyph { rect, uv, advance };

                self.row_height = self.row_height.max(padded_h);
                self.write_glyph(
                    device,
                    Rect::new(x + padding, y + padding, glyph_w, glyph_h).round(),
                    bitmap,
                    atlas,
                );
                font.glyphs.insert(entry, glyph);
                font.add_metrics(f32::from_bits(entry.size));
            }
        }
    }

    fn advance(&mut self, width: f32, height: f32) -> Option<Vec2> {
        if self.cursor.x + width > self.size.width {
            self.cursor.x = 0.0;
            self.cursor.y += self.row_height;
            self.row_height = 0.0;
        }

        if self.cursor.y + height > self.size.height {
            return None;
        }

        let pos = Vec2::new(self.cursor.x.floor(), self.cursor.y.floor());
        self.cursor.x += width;

        Some(pos)
    }

    fn create_atlas(&self, device: &RenderDevice) -> GpuTexture {
        use renderer::wgpu::{
            Extent3d, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages,
        };

        GpuTexture::new(
            device,
            &TextureDescriptor {
                label: Some("glyph_atlas"),
                size: Extent3d {
                    width: self.size.width as u32,
                    height: self.size.height as u32,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: TextureDimension::D2,
                format: TextureFormat::R8Uint,
                usage: TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
            SamplerCache::DEFAULT,
        )
    }

    fn write_glyph(
        &mut self,
        device: &RenderDevice,
        rect: Rect<u32>,
        bitmap: Vec<u8>,
        texture: &mut GpuTexture,
    ) {
        use renderer::wgpu::{
            Extent3d, Origin3d, TexelCopyBufferLayout, TexelCopyTextureInfo, TextureAspect,
        };
        device.queue().write_texture(
            TexelCopyTextureInfo {
                texture,
                mip_level: 0,
                origin: Origin3d {
                    x: rect.x,
                    y: rect.y,
                    z: 0,
                },
                aspect: TextureAspect::All,
            },
            &bitmap,
            TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(rect.width),
                rows_per_image: None,
            },
            Extent3d {
                width: rect.width,
                height: rect.height,
                depth_or_array_layers: 1,
            },
        );
    }
}
