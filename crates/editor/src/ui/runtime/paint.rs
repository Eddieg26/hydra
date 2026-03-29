use crate::ui::{core::id::ElementId, runtime::tree::ElementTree};
use math::rect::Rect;

pub enum DrawCommand {
    PushClip(Rect),
    PopClip,
    Rect(Rect),
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

        self.commands.push(node.element.draw(style, layout));

        if let Some(id) = node.first_child {
            self.paint(id);
        }

        // pop clip

        if let Some(id) = node.next_sibling {
            self.paint(id);
        }
    }

    pub fn finish(self) -> Vec<DrawCommand> {
        self.commands
    }
}
