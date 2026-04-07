use crate::ui::{
    core::{id::ElementId, style::Border, font::Glyph},
    runtime::tree::ElementTree,
};
use asset::AssetId;
use math::rect::Rect;
use renderer::{resources::Texture, types::Color};

pub enum DrawCommand {
    PushClip(Rect),
    PopClip,
    Quad {
        rect: Rect,
        color: Color,
    },
    Text {
        rect: Rect,
        atlas: AssetId<Texture>,
        glyphs: Vec<Glyph>,
    },
    Border {
        rect: Rect,
        border: Border,
    },
    Image {
        rect: Rect,
        id: AssetId<Texture>,
    },
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
            self.commands.push(DrawCommand::Border {
                rect: layout.border,
                border,
            });
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
