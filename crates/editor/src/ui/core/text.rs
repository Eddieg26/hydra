use crate::ui::core::style::{ComputedStyle, TextStyle};
use math::{Vec2, rect::Rect};

pub struct TextMeasurement {
    pub size: Vec2,
    pub line_count: u32,
}

pub trait TextMeasurer {
    fn measure(&self, text: &str, style: &ComputedStyle, max_width: Option<f32>)
    -> TextMeasurement;
}

pub struct TextRequest<'a> {
    pub text: &'a str,
    pub style: &'a TextStyle,
    pub bounds: &'a Rect,
    pub wrap: bool,
}

pub struct UITextSystem {

    
}
