use crate::ui::core::style::ComputedStyle;
use math::{Vec2, rect::Rect};

pub struct TextMeasureResult {
    pub size: Vec2,
}

pub trait TextMeasurer {
    fn measure(
        &self,
        text: &str,
        style: &ComputedStyle,
        max_width: Option<f32>,
    ) -> TextMeasureResult;
}

pub struct Glyph {
    pub position: Vec2,
    pub advance: Vec2,
    pub uv: Rect,
}
