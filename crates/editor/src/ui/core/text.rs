use crate::ui::core::style::ComputedStyle;

pub struct TextBuffer;

pub struct TextMeasureResult;

pub trait TextMeasurer {
    fn measure(
        &mut self,
        text: &TextBuffer,
        style: &ComputedStyle,
        max_width: Option<f32>,
    ) -> TextMeasureResult;
}
