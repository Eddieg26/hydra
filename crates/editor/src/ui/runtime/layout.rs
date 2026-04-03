use crate::ui::{
    core::{
        id::ElementId,
        image::ImageResolver,
        style::{
            Align, ComputedStyle, Constrained, Edges, Flex, FlexDirection, FlexWrap, Justify,
            Length, Overflow,
        },
        text::TextMeasurer,
    },
    runtime::tree::ElementTree,
};
use math::{Size, rect::Rect};
use std::ops::Range;

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
        let style = &self.tree.computed_styles[id];
        let border_width = style.border.map(|b| b.width).unwrap_or(0.0);
        let content = Rect {
            x: rect.x + border_width + style.padding.left,
            y: rect.y + border_width + style.padding.top,
            width: (rect.width - style.padding.horizontal()).max(0.0) - border_width,
            height: (rect.height - style.padding.vertical()).max(0.0) - border_width,
        };
        let direction = style.flex.direction;
        let axis = FlexAxis::pair(style, content);

        let mut items = Vec::with_capacity(node.children.len());
        for child in &node.children {
            let child_node = &self.tree.nodes[*child];
            let child_style = &self.tree.computed_styles[*child];
            let intrinisic = child_node
                .element
                .measure(&self.resolver)
                .unwrap_or(Size::ZERO);

            let width = child_style
                .width
                .resolve(intrinisic.width, Some(content.width));

            let height = child_style
                .height
                .resolve(intrinisic.height, Some(content.height));

            items.push(FlexItem::new(
                *child,
                child_style,
                width,
                height,
                child_style.margin,
                Flex {
                    direction,
                    grow: child_style.flex.grow,
                    shrink: child_style.flex.shrink,
                },
            ));
        }

        let mut lines = Vec::new();
        let mut current = FlexLine::default();
        for index in 0..items.len() {
            let item = &mut items[index];
            let gap = if index == current.items.start {
                0.0
            } else {
                axis.main.gap
            };
            let item_main_size =
                item.base_size.main.clamp() + item.margin.main.0 + item.margin.main.1;
            let item_cross_size =
                item.base_size.cross.clamp() + item.margin.cross.0 + item.margin.cross.1;
            let mut offset = current.size.main + gap + item_main_size;

            if offset > axis.main.space && style.wrap == FlexWrap::Wrap {
                lines.push(current);
                current = FlexLine::new(index);
                offset = item_main_size;
            }

            current.items.end = index + 1;
            current.size.main = offset;
            current.size.cross = current.size.cross.max(item_cross_size);
            current.grow += item.grow;
            current.shrink += item.shrink;
            current.weight += item.weight;
        }

        if !current.items.is_empty() {
            lines.push(current);
        }

        let mut cursor = FlexCursor::new(style, &lines, &axis.cross, content);
        for line in lines {
            let mut cursor = cursor.start(style.justify, &line, &axis.main);

            for index in line.items {
                let item = &mut items[index];

                item.final_size.main = if cursor.free_space > 0.0 {
                    let extra = cursor.free_space * (item.grow / line.grow.max(1.0));
                    item.base_size
                        .main
                        .with(item.base_size.main.clamp() + extra)
                        .clamp()
                } else if cursor.free_space < 0.0 && line.weight > 0.0 {
                    let loss = -cursor.free_space * (item.weight / line.weight);
                    item.base_size
                        .main
                        .with(item.base_size.main.clamp() - loss)
                        .clamp()
                } else {
                    item.base_size.main.clamp()
                };

                item.final_size.cross = match (style.align, item.length.cross) {
                    (Align::Stretch, Length::Auto) => item
                        .base_size
                        .cross
                        .with(line.size.cross - item.margin.cross.0 - item.margin.cross.1)
                        .clamp(),
                    _ => item.base_size.cross.clamp(),
                };

                let free_cross_space = line.size.cross
                    - item.final_size.cross
                    - item.margin.cross.0
                    - item.margin.cross.1;

                let cursor_offset = match style.align {
                    Align::Center => free_cross_space * 0.5,
                    Align::End => free_cross_space,
                    _ => 0.0,
                };

                item.final_pos.main = cursor.offset.main + item.margin.main.0;
                item.final_pos.cross = cursor.offset.cross + item.margin.cross.0 + cursor_offset;

                cursor.next(item);
            }

            cursor.finish(line.size.cross);
        }

        let clip = {
            let parent = node
                .parent
                .map(|p| &self.tree.layouts[p])
                .map(|l| l.outer)
                .unwrap_or(content);

            let (x, width) = match style.overflow_x {
                Overflow::Visible => (parent.x, parent.width),
                Overflow::Hidden | Overflow::Scroll => {
                    let area = content.intersect(&parent);
                    (area.x, area.width)
                }
            };

            let (y, height) = match style.overlfow_y {
                Overflow::Visible => (parent.y, parent.height),
                Overflow::Hidden | Overflow::Scroll => {
                    let area = content.intersect(&parent);
                    (area.y, area.height)
                }
            };

            Rect {
                x,
                y,
                width,
                height,
            }
        };

        let border = Rect {
            x: rect.x,
            y: rect.y,
            width: border_width,
            height: border_width,
        };

        self.tree.layouts.insert(
            id,
            Layout {
                outer: rect,
                content,
                border,
                clip,
                scroll: Rect::ZERO,
            },
        );

        let mut content_size = Size::ZERO;
        for item in items {
            let id = item.element;
            let rect = item.rect(direction);
            let width = (rect.x + rect.width) - content.x;
            let height = (rect.y + rect.height) - content.y;
            content_size = content_size.max(width, height);

            self.run(id, rect);
        }

        self.tree.layouts[id].scroll = Rect {
            x: 0.0,
            y: 0.0,
            width: (content_size.width - clip.width).max(0.0),
            height: (content_size.height - clip.height).max(0.0),
        }
    }
}

pub struct FlexCursor {
    pub offset: FlexValue<f32>,
    pub spacing: FlexValue<f32>,
}

impl FlexCursor {
    pub fn new(style: &ComputedStyle, lines: &[FlexLine], axis: &FlexAxis, parent: Rect) -> Self {
        let gap_space = (lines.len().saturating_sub(1)) as f32 * axis.gap;
        let used_space = lines.iter().map(|l| l.size.cross).sum::<f32>() + gap_space;
        let free_space = axis.space - used_space;

        let (main_start, cross_start) = match style.flex.direction {
            FlexDirection::Row => (parent.x, parent.y),
            FlexDirection::Column => (parent.y, parent.x),
        };

        let (cross_offset, cross_spacing) = match style.align {
            Align::Start => (0.0, axis.gap),
            Align::Center => ((free_space * 0.5).max(0.0), axis.gap),
            Align::End => (free_space.max(0.0), axis.gap),
            Align::Stretch => (0.0, axis.gap),
        };

        Self {
            offset: FlexValue::new(main_start, cross_start + cross_offset),
            spacing: FlexValue::new(0.0, cross_spacing),
        }
    }

    pub fn start<'a>(
        &'a mut self,
        justify: Justify,
        line: &FlexLine,
        axis: &FlexAxis,
    ) -> LineCursor<'a> {
        LineCursor::new(self, justify, line, axis)
    }
}

pub struct LineCursor<'a> {
    cursor: &'a mut FlexCursor,
    free_space: f32,
}

impl<'a> LineCursor<'a> {
    pub fn new(
        cursor: &'a mut FlexCursor,
        justify: Justify,
        line: &FlexLine,
        axis: &FlexAxis,
    ) -> Self {
        let free_space = axis.space - line.size.main;

        let (offset, spacing) = match justify {
            Justify::Start => (0.0, axis.gap),
            Justify::Center => ((free_space * 0.5).max(0.0), axis.gap),
            Justify::End => (free_space.max(0.0), axis.gap),
            Justify::Between => {
                let gap = if line.items.len() > 1 {
                    axis.gap + free_space.max(0.0) / (line.items.len() - 1) as f32
                } else {
                    0.0
                };

                (0.0, gap)
            }
            Justify::Around => {
                let slot = if line.items.len() > 0 {
                    free_space.max(0.0) / line.items.len() as f32
                } else {
                    0.0
                };

                (slot * 0.5, axis.gap + slot)
            }
            Justify::Evenly => {
                let slot = free_space.max(0.0) / (line.items.len() + 1) as f32;
                (slot, axis.gap + slot)
            }
        };

        cursor.offset.main += offset;
        cursor.spacing.main = spacing;

        Self { cursor, free_space }
    }

    pub fn next(&mut self, item: &FlexItem) {
        self.offset.main +=
            item.final_size.main + item.margin.main.0 + item.margin.main.1 + self.spacing.main;
    }

    pub fn finish(self, size: f32) {
        self.cursor.offset.cross += self.cursor.spacing.cross + size
    }
}

impl std::ops::Deref for LineCursor<'_> {
    type Target = FlexCursor;

    fn deref(&self) -> &Self::Target {
        &self.cursor
    }
}

impl std::ops::DerefMut for LineCursor<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.cursor
    }
}

pub struct FlexValue<T> {
    pub main: T,
    pub cross: T,
}

impl<T> FlexValue<T> {
    pub fn new(main: T, cross: T) -> Self {
        Self { main, cross }
    }
}

impl<T: Copy> Copy for FlexValue<T> {}
impl<T: Copy> Clone for FlexValue<T> {
    fn clone(&self) -> Self {
        Self {
            main: self.main.clone(),
            cross: self.cross.clone(),
        }
    }
}

#[derive(Clone, Copy)]
pub struct FlexAxis {
    pub space: f32,
    pub gap: f32,
}

impl FlexAxis {
    pub fn pair(style: &ComputedStyle, rect: Rect) -> FlexValue<FlexAxis> {
        let x = FlexAxis {
            space: rect.width,
            gap: style.gap_x.resolve(0.0, Some(rect.width)),
        };

        let y = FlexAxis {
            space: rect.height,
            gap: style.gap_y.resolve(0.0, Some(rect.height)),
        };

        match style.flex.direction {
            FlexDirection::Row => FlexValue::new(x, y),
            FlexDirection::Column => FlexValue::new(y, x),
        }
    }
}

pub struct FlexItem {
    pub element: ElementId,
    pub base_size: FlexValue<Constrained<f32>>,
    pub final_size: FlexValue<f32>,
    pub final_pos: FlexValue<f32>,
    pub margin: FlexValue<(f32, f32)>,
    pub grow: f32,
    pub shrink: f32,
    pub weight: f32,
    pub length: FlexValue<Length>,
}

impl FlexItem {
    pub fn new(
        element: ElementId,
        style: &ComputedStyle,
        width: Constrained<f32>,
        height: Constrained<f32>,
        margin: Edges<f32>,
        flex: Flex,
    ) -> Self {
        match flex.direction {
            FlexDirection::Row => Self {
                element,
                base_size: FlexValue::new(width, height),
                final_size: FlexValue::new(0.0, 0.0),
                final_pos: FlexValue::new(0.0, 0.0),
                margin: FlexValue::new((margin.left, margin.right), (margin.top, margin.bottom)),
                grow: flex.grow,
                shrink: flex.shrink,
                weight: flex.shrink * width.clamp(),
                length: FlexValue::new(style.width.value, style.height.value),
            },
            FlexDirection::Column => Self {
                element,
                base_size: FlexValue::new(height, width),
                final_size: FlexValue::new(0.0, 0.0),
                final_pos: FlexValue::new(0.0, 0.0),
                margin: FlexValue::new((margin.top, margin.bottom), (margin.left, margin.right)),
                grow: flex.grow,
                shrink: flex.shrink,
                weight: flex.shrink * height.clamp(),
                length: FlexValue::new(style.height.value, style.width.value),
            },
        }
    }

    pub fn rect(self, direction: FlexDirection) -> Rect {
        match direction {
            FlexDirection::Row => Rect {
                x: self.final_pos.main,
                y: self.final_pos.cross,
                width: self.final_size.main,
                height: self.final_size.cross,
            },
            FlexDirection::Column => Rect {
                x: self.final_pos.cross,
                y: self.final_pos.main,
                width: self.final_size.cross,
                height: self.final_size.main,
            },
        }
    }
}

pub struct FlexLine {
    pub items: Range<usize>,
    pub size: FlexValue<f32>,
    pub grow: f32,
    pub shrink: f32,
    pub weight: f32,
}

impl FlexLine {
    pub fn new(start: usize) -> Self {
        Self {
            items: start..start,
            size: FlexValue::new(0.0, 0.0),
            grow: 0.0,
            shrink: 0.0,
            weight: 0.0,
        }
    }
}

impl Default for FlexLine {
    fn default() -> Self {
        Self::new(0)
    }
}

pub struct Layout {
    pub outer: Rect,
    pub content: Rect,
    pub border: Rect,
    pub clip: Rect,
    pub scroll: Rect,
}
