use math::Vec2;

use crate::ui::{
    core::{id::ElementId, style::ComputedStyle},
    runtime::{
        layout::{ContentResolver, Layout},
        paint::DrawCommand,
    },
};

pub trait Element: Send + Sync + 'static {
    fn measure(&self, available: Vec2, _resolver: &ContentResolver) -> Vec2 {
        available
    }

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
    pub children: Vec<ElementId>,
    pub element: Box<dyn Element>,
    pub flags: ElementFlags,
}
