use crate::ui::{
    core::{ImageHandle, ImageResolver, id::ElementId, style::ComputedStyle},
    runtime::{layout::Layout, paint::DrawCommand, text::TextResolver},
};
use math::Size;

pub struct ElementResolver<'a> {
    text: &'a mut dyn TextResolver,
    image: &'a dyn ImageResolver,
}

impl<'a> ElementResolver<'a> {
    pub fn new(text: &'a mut dyn TextResolver, image: &'a dyn ImageResolver) -> Self {
        Self { text, image }
    }

    pub fn image_size(&self, image: ImageHandle) -> Size {
        self.image.size(image)
    }

    pub fn measure_text(&mut self, id: ElementId, text: &str, style: &ComputedStyle) -> Size {
        self.text.measure(id, text, style)
    }
}

pub trait Element: Send + Sync + 'static {
    fn measure(&self, _id: ElementId, _resolver: &mut ElementResolver) -> Option<Size> {
        None
    }

    fn draw(
        &self,
        id: ElementId,
        resolver: &mut ElementResolver,
        style: &ComputedStyle,
        layout: &Layout,
    ) -> DrawCommand;
}

bitflags::bitflags! {
    pub struct ElementFlags: u32 {
        const STRUCTURE = 1 << 0;
        const STYLE = 1 << 1;
        const LAYOUT = 1 << 2;
        const PAINT = 1 << 3;
        const HIT_TEST = 1 << 4;
    }
}

pub struct ElementNode {
    pub id: ElementId,
    pub parent: Option<ElementId>,
    pub children: Vec<ElementId>,
    pub element: Box<dyn Element>,
    pub flags: ElementFlags,
}
