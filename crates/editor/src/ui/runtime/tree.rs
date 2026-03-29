use crate::ui::{
    core::{
        id::ElementId,
        style::{ComputedStyle, ElementStyle},
    },
    runtime::{layout::Layout, node::ElementNode},
};
use slotmap::{SecondaryMap, SlotMap};

pub struct ElementTree {
    nodes: SlotMap<ElementId, ElementNode>,
    element_styles: SecondaryMap<ElementId, ElementStyle>,
    computed_styles: SecondaryMap<ElementId, ComputedStyle>,
    layouts: SecondaryMap<ElementId, Layout>,
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
