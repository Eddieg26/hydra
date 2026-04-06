use asset::{Asset, AssetId};
use math::{Size, Vec2, rect::Rect};
use renderer::{
    core::RenderDevice,
    resources::{GpuTexture, RenderAsset, RenderAssets, Texture},
};
use std::collections::{HashMap, HashSet};

#[derive(Clone, Asset)]
pub struct Font {}

pub struct GpuFont(fontdue::Font);

impl RenderAsset for GpuFont {
    type Asset = Font;

    type Arg = ();

    fn extract(
        id: AssetId<Self::Asset>,
        asset: Self::Asset,
        arg: &mut ecs::ArgItem<Self::Arg>,
    ) -> Result<Self, renderer::resources::ExtractError<Self::Asset>> {
        todo!()
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
        fonts: &RenderAssets<GpuFont>,
        queue: &mut GlyphQueue,
        textures: &mut RenderAssets<GpuTexture>,
    ) {
        let mut buffer = Vec::new();
        for (id, entries) in queue.0.drain() {
            let Some(font) = fonts.get(&id) else {
                continue;
            };

            for entry in entries {
                let (metrics, bitmap) = font.0.rasterize(entry.ch, entry.size as f32);

                let width = self.padding * 2.0 + metrics.width as f32;
                let height = self.padding * 2.0 + metrics.height as f32;

                let Some(Vec2 { x, y }) = self.advance(width, height) else {
                    continue;
                };

                let rect = Rect::new(metrics.xmin as f32, metrics.ymin as f32, width, height);
                let uv = Rect::new(
                    x / self.size.width,
                    y / self.size.height,
                    width / self.size.width,
                    height / self.size.height,
                );
                let advance = Vec2::new(metrics.advance_width, metrics.advance_height);

                let key = GlyphKey::new(id, entry.ch, entry.size);
                let glyph = Glyph { rect, uv, advance };

                buffer.extend(bitmap);
                self.row_height = self.row_height.max(height);
                self.glyphs.insert(key, glyph);
            }
        }

        if buffer.is_empty() {
            return;
        }

        let atlas = match textures.get_mut(&GlyphAtlas::ID) {
            Some(atlas) => atlas,
            None => {
                // Create texture
                textures.get_mut(&GlyphAtlas::ID).unwrap()
            }
        };

        // Write data into atlas
    }

    fn advance(&mut self, width: f32, height: f32) -> Option<Vec2> {
        if self.cursor.x + width > self.size.width {
            self.cursor.x = 0.0;
            self.cursor.y += self.row_height;
            self.row_height = 0.0;
        }

        if self.cursor.y + height > self.row_height {
            return None;
        }

        let x = self.cursor.x * self.padding;
        let y = self.cursor.y * self.padding;

        Some(Vec2::new(x, y))
    }
}
