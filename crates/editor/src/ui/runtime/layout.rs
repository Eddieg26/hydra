use crate::ui::{
    core::{id::ElementId, text::TextMeasurer},
    runtime::tree::ElementTree,
};
use math::{Size, rect::Rect};

pub struct LayoutEngine<'a> {
    tree: &'a mut ElementTree,
    text_measurer: &'a mut dyn TextMeasurer,
}

impl<'a> LayoutEngine<'a> {
    pub fn run(&mut self, root: ElementId, size: Size) {
        // resolve dirty subtrees only
    }
}

pub struct Layout {
    pub outer: Rect,
    pub content: Rect,
    pub border: Rect,
    pub clip: Rect,
    pub scroll: Size,
}
