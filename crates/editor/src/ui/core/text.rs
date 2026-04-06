use crate::ui::core::style::{ComputedStyle, TextStyle};
use asset::{Asset, AssetId};
use math::{Size, Vec2, rect::Rect};
use renderer::resources::Texture;
use std::collections::HashMap;

pub struct TextMeasurement {
    pub size: Vec2,
    pub line_count: u32,
}

pub trait TextMeasurer {
    fn measure(&self, text: &str, style: &ComputedStyle, max_width: Option<f32>)
    -> TextMeasurement;
}

pub struct Glyph {
    pub rect: Rect,
    pub uv: Rect,
    pub advance: Vec2,
}

#[derive(Asset, Clone)]
pub struct UIFont(fontdue::Font);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GlyphKey {
    pub font: AssetId<UIFont>,
    pub ch: char,
    pub size: u32,
}

pub struct GlyphAtlas {
    cursor: Vec2,
    size: Size,
    row_height: u32,
    padding: u32,
    entries: HashMap<GlyphKey, Glyph>,
}

impl GlyphAtlas {
    pub const ID: AssetId<Texture> = AssetId::from_u128(0);

}

pub struct TextRequest<'a> {
    pub text: &'a str,
    pub style: &'a TextStyle,
    pub bounds: &'a Rect,
    pub wrap: bool,
}

pub struct UITextSystem {}
