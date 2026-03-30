use crate::ui::{
    core::{id::ElementId, style::Border, text::Glyph},
    runtime::tree::ElementTree,
};
use math::{Vec2, rect::Rect};
use renderer::types::Color;

pub enum DrawCommand {
    PushClip(Rect),
    PopClip,
    Quad {
        rect: Rect,
        color: Color,
    },
    Text {
        glyphs: Vec<Glyph>,
        positions: Vec<Vec2>,
    },
    Border(Border),
    Image,
}

pub struct Painter<'a> {
    tree: &'a ElementTree,
    commands: Vec<DrawCommand>,
}

impl<'a> Painter<'a> {
    pub fn new(tree: &'a ElementTree) -> Self {
        Self {
            tree,
            commands: Vec::new(),
        }
    }

    pub fn paint(&mut self, id: ElementId) {
        let Some(node) = self.tree.node(id) else {
            return;
        };

        let Some(layout) = self.tree.layout(id) else {
            return;
        };

        let Some(style) = self.tree.computed(id) else {
            return;
        };

        // push clip
        if let Some(border) = style.border.as_ref().copied() {
            self.commands.push(DrawCommand::Border(border));
        }

        self.commands.push(node.element.draw(style, layout));

        for child in &node.children {
            self.paint(*child);
        }
        // pop clip
    }

    pub fn finish(self) -> Vec<DrawCommand> {
        self.commands
    }
}
