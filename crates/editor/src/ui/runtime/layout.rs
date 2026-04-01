use std::ops::Range;

use crate::ui::{
    core::{
        id::ElementId,
        image::ImageResolver,
        style::{Align, ComputedStyle, Constrained, Edges, Flex, FlexDirection, Justify},
        text::TextMeasurer,
    },
    runtime::tree::ElementTree,
};
use math::{Vec2, rect::Rect};

pub struct ContentResolver<'a> {
    pub text: &'a dyn TextMeasurer,
    pub image: &'a dyn ImageResolver,
}

pub struct LayoutEngine<'a> {
    tree: &'a mut ElementTree,
    resolver: ContentResolver<'a>,
}

impl<'a> LayoutEngine<'a> {
    pub fn run(&mut self, id: ElementId, rect: Rect) {
        let node = &self.tree.nodes[id];
        if node.children.is_empty() {
            return;
        }

        let style = &self.tree.computed_styles[id];
        let content_rect = Rect {
            x: rect.x + style.padding.left,
            y: rect.y + style.padding.top,
            width: (rect.width - style.padding.horizontal()).max(0.0)
                - style.border.map(|b| b.width.horizontal()).unwrap_or(0.0),
            height: (rect.height - style.padding.vertical()).max(0.0)
                - style.border.map(|b| b.width.vertical()).unwrap_or(0.0),
        };

        let mut container = FlexContainer::new(style, content_rect);
        let mut items = Vec::with_capacity(node.children.len());

        for child in &node.children {
            let child = &self.tree.nodes[*child];
            let child_style = &self.tree.computed_styles[child.id];
            let intrinsic = child.element.measure(&self.resolver).unwrap_or(Vec2::ZERO);

            let width = child_style
                .width
                .resolve(intrinsic.x, Some(content_rect.width));
            let height = child_style
                .height
                .resolve(intrinsic.y, Some(content_rect.height));

            items.push(FlexItem::new(
                child.id,
                width,
                height,
                child_style.margin,
                Flex {
                    direction: style.flex.direction,
                    grow: child_style.flex.grow,
                    shrink: child_style.flex.shrink,
                },
            ));
        }

        container.main_gap_space = (items.len() - 1) as f32 * container.main_gap;
        container.used_space = container.main_gap
            + items
                .iter()
                .map(|item| item.main_size.value + item.margin_main_start + item.margin_main_end)
                .sum::<f32>();
        container.free_space = container.total_main_space - container.used_space;

        if container.free_space > 0.0 {
        } else if container.free_space < 0.0 {
        }

        let mut lines = Vec::new();
        let mut current_line = FlexLine::default();

        for index in 0..items.len() {
            let item = &mut items[index];
            let gap = current_line.gap(index, container.main_gap);
            let offset = current_line.main_size + gap + item.main_size.clamp();

            if offset > container.total_main_space {
                lines.push(current_line);
                current_line = FlexLine::new(index);
            } else {
                current_line.items.end = index + 1;
                current_line.main_size = offset;
            }
        }

        if !current_line.items.is_empty() {
            lines.push(current_line);
        }

        let mut cursor = FlexCursor::new(style, &container);
        for mut line in lines {
            // Position each item and determine cross axis size
            for index in line.items {}
        }
    }
}

#[derive(Debug)]
pub struct FlexContainer {
    pub used_space: f32,
    pub free_space: f32,
    pub main_gap: f32,
    pub cross_gap: f32,
    pub main_gap_space: f32,
    pub cross_gap_space: f32,
    pub total_main_space: f32,
    pub total_cross_space: f32,
    pub total_grow: f32,
    pub total_shrink: f32,
}

impl FlexContainer {
    pub fn new(style: &ComputedStyle, rect: Rect) -> Self {
        let (main_space, cross_space) = match style.flex.direction {
            FlexDirection::Row => (rect.width, rect.height),
            FlexDirection::Column => (rect.height, rect.width),
        };

        let (main_gap, cross_gap) = match style.flex.direction {
            FlexDirection::Row => (style.gap_x, style.gap_y),
            FlexDirection::Column => (style.gap_y, style.gap_x),
        };

        Self {
            main_gap: main_gap.resolve(0.0, Some(main_space)),
            cross_gap: cross_gap.resolve(0.0, Some(cross_space)),
            used_space: 0.0,
            main_gap_space: 0.0,
            cross_gap_space: 0.0,
            free_space: 0.0,
            total_main_space: main_space,
            total_cross_space: cross_space,
            total_grow: 0.0,
            total_shrink: 0.0,
        }
    }
}

pub struct FlexCursor {
    main_offset: f32,
    cross_offset: f32,
    main_start: f32,
    main_spacing: f32,
    cross_start: f32,
    cross_spacing: f32,
    justify: Justify,
    align: Align,
}

impl FlexCursor {
    pub fn new(style: &ComputedStyle, container: &FlexContainer) -> Self {
        todo!()
    }

    pub fn start_line(&mut self, line: &FlexLine, container: &FlexContainer, items: usize) {
        let (main_start, main_spacing) = match self.justify {
            Justify::Start => (0.0, container.main_gap),
            Justify::Center => ((container.free_space * 0.5).max(0.0), container.main_gap),
            Justify::End => (container.free_space.max(0.0), container.main_gap),
            Justify::Between => {
                let spacing = if items > 1 {
                    container.main_gap + container.free_space.max(0.0) / (items as f32 - 1.0)
                } else {
                    0.0
                };

                (0.0, spacing)
            }
            Justify::Around => {
                let slot = if items > 0 {
                    container.free_space.max(0.0) / items as f32
                } else {
                    0.0
                };

                (slot * 0.5, container.main_gap + slot)
            }
            Justify::Evenly => {
                let slot = container.free_space.max(0.0) / (items as f32 + 1.0);
                (slot, container.main_gap + slot)
            }
        };

        let (cross_start, cross_spacing) = match self.align {
            Align::Start => (0.0, container.cross_gap),
            Align::Center => todo!(),
            Align::End => todo!(),
            Align::Stretch => todo!(),
        };

        self.main_start = main_start;
        self.main_spacing = main_spacing;
        self.cross_start = cross_start;
        self.cross_spacing = cross_spacing;
    }
}

#[derive(Default)]
pub struct FlexLine {
    items: Range<usize>,
    main_size: f32,
    cross_size: f32,
}

impl FlexLine {
    pub fn new(start: usize) -> Self {
        Self {
            items: start..start,
            main_size: 0.0,
            cross_size: 0.0,
        }
    }

    #[inline]
    pub fn gap(&self, index: usize, value: f32) -> f32 {
        if index == self.items.start {
            0.0
        } else {
            value
        }
    }
}

pub struct FlexItem {
    element: ElementId,
    main_size: Constrained<f32>,
    cross_size: Constrained<f32>,

    margin_main_start: f32,
    margin_main_end: f32,
    margin_cross_start: f32,
    margin_cross_end: f32,

    grow: f32,
    shrink: f32,

    final_main_size: f32,
    final_cross_size: f32,

    main_pos: f32,
    cross_pos: f32,
}

impl FlexItem {
    fn new(
        id: ElementId,
        width: Constrained<f32>,
        height: Constrained<f32>,
        margin: Edges<f32>,
        flex: Flex,
    ) -> Self {
        match flex.direction {
            FlexDirection::Row => Self {
                element: id,
                main_size: width,
                cross_size: height,
                margin_main_start: margin.left,
                margin_main_end: margin.right,
                margin_cross_start: margin.top,
                margin_cross_end: margin.bottom,
                grow: flex.grow,
                shrink: flex.shrink,
                final_main_size: 0.0,
                final_cross_size: 0.0,
                main_pos: 0.0,
                cross_pos: 0.0,
            },
            FlexDirection::Column => Self {
                element: id,
                main_size: height,
                cross_size: width,
                margin_main_start: margin.top,
                margin_main_end: margin.bottom,
                margin_cross_start: margin.left,
                margin_cross_end: margin.right,
                grow: flex.grow,
                shrink: flex.shrink,
                final_main_size: 0.0,
                final_cross_size: 0.0,
                main_pos: 0.0,
                cross_pos: 0.0,
            },
        }
    }
}

pub struct Layout {
    pub outer: Rect,
    pub content: Rect,
    pub border: Rect,
    pub clip: Rect,
    pub scroll: Vec2,
}
