use crate::ui::{
    core::{
        id::ElementId,
        style::{ComputedStyle, ElementStyle},
    },
    runtime::{layout::Layout, node::ElementNode},
};
use slotmap::{SecondaryMap, SlotMap};

pub struct ElementTree {
    pub(crate) nodes: SlotMap<ElementId, ElementNode>,
    pub(crate) element_styles: SecondaryMap<ElementId, ElementStyle>,
    pub(crate) computed_styles: SecondaryMap<ElementId, ComputedStyle>,
    pub(crate) layouts: SecondaryMap<ElementId, Layout>,
}

impl ElementTree {
    pub fn node(&self, id: ElementId) -> Option<&ElementNode> {
        self.nodes.get(id)
    }

    pub fn style(&self, id: ElementId) -> Option<&ElementStyle> {
        self.element_styles.get(id)
    }

    pub fn computed(&self, id: ElementId) -> Option<&ComputedStyle> {
        self.computed_styles.get(id)
    }

    pub fn layout(&self, id: ElementId) -> Option<&Layout> {
        self.layouts.get(id)
    }
}
