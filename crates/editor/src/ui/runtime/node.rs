use crate::ui::{
    core::{id::ElementId, style::ComputedStyle},
    runtime::{layout::Layout, paint::DrawCommand},
};

pub trait Element: Send + Sync + 'static {
    fn draw(&self, style: &ComputedStyle, layout: &Layout) -> DrawCommand;
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
    pub first_child: Option<ElementId>,
    pub next_sibling: Option<ElementId>,
    pub prev_sibling: Option<ElementId>,
    pub element: Box<dyn Element>,
    pub flags: ElementFlags,
}
