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
    runtime::{node::ElementNode, tree::ElementTree},
};
use math::{Size, rect::Rect};
use std::{collections::VecDeque, ops::Range};

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
        let content = Rect::new(
            rect.x + border_width,
            rect.y + border_width,
            rect.width - border_width - style.padding.horizontal(),
            rect.height - border_width - style.padding.vertical(),
        );
        let clip = Layout::clip(
            style,
            &content,
            node.parent.map(|p| &self.tree.layouts[p].outer),
        );

        self.tree
            .layouts
            .insert(id, Layout::new(rect, content, clip, border_width));

        let axis = FlexAxis::pair(style, content);

        let mut items = self.create_items(node, style, &content);
        let mut lines = self.create_lines(style.wrap, &axis.main, &mut items);

        self.pack_main_axis(&axis.main, &mut items, &mut lines);
        self.pack_cross_axis(style.align, &mut items, &mut lines);

        let rects = self.position_items(style, &rect, &axis, lines, items);
        self.tree.layouts[id].scroll = self.layout_children(&content, &clip, rects);
    }

    fn create_items(
        &self,
        node: &ElementNode,
        style: &ComputedStyle,
        content: &Rect,
    ) -> Vec<FlexItem> {
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
                    direction: style.flex.direction,
                    grow: child_style.flex.grow,
                    shrink: child_style.flex.shrink,
                },
            ));
        }

        items
    }

    fn create_lines(
        &self,
        wrap: FlexWrap,
        axis: &FlexAxis,
        items: &mut [FlexItem],
    ) -> Vec<FlexLine> {
        let mut lines = Vec::new();
        let mut current = FlexLine::default();

        for index in 0..items.len() {
            let item = &mut items[index];
            let gap = if index == current.items.start {
                0.0
            } else {
                axis.gap
            };
            let item_main_size = item.base_size.main.clamp() + item.margin.main.size();
            let item_cross_size = item.base_size.cross.clamp() + item.margin.cross.size();
            let mut offset = current.size.main + gap + item_main_size;

            if offset > axis.space && wrap == FlexWrap::Wrap {
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

        lines
    }

    fn pack_main_axis(&self, axis: &FlexAxis, items: &mut [FlexItem], lines: &mut [FlexLine]) {
        fn inner(axis: &FlexAxis, items: &mut [FlexItem], line: &mut FlexLine) {
            let mut unfrozen = line.items.clone().collect::<Vec<_>>();

            loop {
                let free_space = axis.space - line.size.main;
                let partition = free_space.abs();
                let mut used_space = 0.0;

                for index in std::mem::take(&mut unfrozen) {
                    let item = &mut items[index];
                    let (item_size, frozen) = if free_space > 0.0 {
                        let extra = partition * (item.grow / line.grow.max(1.0));
                        item.base_size
                            .main
                            .with(item.base_size.main.value + extra)
                            .clamped()
                    } else if free_space < 0.0 && line.weight > 0.0 {
                        let loss = partition * (item.weight / line.weight);
                        item.base_size
                            .main
                            .with(item.base_size.main.value - loss)
                            .clamped()
                    } else {
                        item.base_size.main.clamped()
                    };

                    if frozen {
                        item.final_size.main = item_size;
                        used_space += item_size + item.margin.main.size();
                        line.grow = line.grow - item.grow;
                        line.shrink = line.shrink - item.shrink;
                        line.weight = line.weight - item.weight;
                    } else {
                        used_space += item.base_size.main.clamp() + item.margin.main.size();
                        unfrozen.push(index);
                    }
                }

                line.size.main = used_space + line.items.len().saturating_sub(1) as f32 * axis.gap;

                if unfrozen.is_empty() || partition <= f32::EPSILON {
                    break;
                }
            }
        }

        for line in lines {
            inner(axis, items, line)
        }
    }

    fn pack_cross_axis(&self, align: Align, items: &mut [FlexItem], lines: &mut [FlexLine]) {
        for line in lines {
            for index in line.items.clone() {
                let item = &mut items[index];

                item.final_size.cross = match (align, item.length.cross) {
                    (Align::Stretch, Length::Auto) => item
                        .base_size
                        .cross
                        .with(line.size.cross - item.margin.cross.start - item.margin.cross.end)
                        .clamp(),
                    _ => item.base_size.cross.clamp(),
                };
            }
        }
    }

    fn position_items(
        &self,
        style: &ComputedStyle,
        parent: &Rect,
        axis: &FlexValue<FlexAxis>,
        lines: Vec<FlexLine>,
        items: Vec<FlexItem>,
    ) -> Vec<(ElementId, Rect)> {
        let mut cursor = FlexCursor::new(style, parent, axis, &lines);
        let mut rects = Vec::with_capacity(items.len());

        for line in lines {
            cursor.start(style.justify, parent, axis, &line);

            for index in line.items {
                let item = &items[index];
                let rect = match style.flex.direction {
                    FlexDirection::Row => Rect {
                        x: cursor.offset.main + item.margin.main.start,
                        y: cursor.offset.cross + item.margin.cross.start,
                        width: item.final_size.main,
                        height: item.final_size.cross,
                    },
                    FlexDirection::Column => Rect {
                        x: cursor.offset.cross + item.margin.cross.start,
                        y: cursor.offset.main + item.margin.main.start,
                        width: item.final_size.cross,
                        height: item.final_size.main,
                    },
                };

                rects.push((item.element, rect));

                cursor.next(item);
            }

            cursor.end(line.size.cross);
        }

        rects
    }

    fn layout_children(
        &mut self,
        content: &Rect,
        clip: &Rect,
        rects: Vec<(ElementId, Rect)>,
    ) -> Rect {
        let mut content_size = Size::ZERO;

        for (id, rect) in rects {
            let width = (rect.x + rect.width) - content.x;
            let height = (rect.y + rect.height) - content.y;
            content_size = content_size.max(width, height);

            self.run(id, rect);
        }

        Rect {
            x: 0.0,
            y: 0.0,
            width: (content_size.width - clip.width).max(0.0),
            height: (content_size.height - clip.height).max(0.0),
        }
    }
}

pub struct Layout {
    pub outer: Rect,
    pub content: Rect,
    pub border: Rect,
    pub clip: Rect,
    pub scroll: Rect,
}

impl Layout {
    pub fn new(outer: Rect, content: Rect, clip: Rect, border: f32) -> Self {
        Self {
            outer,
            content,
            border: Rect {
                x: outer.x,
                y: outer.y,
                width: border,
                height: border,
            },
            clip,
            scroll: Rect::ZERO,
        }
    }

    fn clip(style: &ComputedStyle, content: &Rect, parent: Option<&Rect>) -> Rect {
        let parent = parent.unwrap_or(content);

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
    }
}

pub struct FlexValue<T> {
    main: T,
    cross: T,
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

pub struct FlexBlock<T> {
    start: T,
    end: T,
}

impl<T> FlexBlock<T> {
    pub fn new(start: T, end: T) -> Self {
        Self { start, end }
    }
}

impl<T: Copy> Copy for FlexBlock<T> {}
impl<T: Copy> Clone for FlexBlock<T> {
    fn clone(&self) -> Self {
        Self {
            start: self.start.clone(),
            end: self.end.clone(),
        }
    }
}

impl FlexBlock<f32> {
    pub fn size(&self) -> f32 {
        self.start + self.end
    }
}

pub struct FlexItem {
    pub element: ElementId,
    pub base_size: FlexValue<Constrained<f32>>,
    pub final_size: FlexValue<f32>,
    pub margin: FlexValue<FlexBlock<f32>>,
    pub length: FlexValue<Length>,
    pub grow: f32,
    pub shrink: f32,
    pub weight: f32,
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
                margin: FlexValue::new(
                    FlexBlock::new(margin.left, margin.right),
                    FlexBlock::new(margin.top, margin.bottom),
                ),
                length: FlexValue::new(style.width.value, style.height.value),
                grow: flex.grow,
                shrink: flex.shrink,
                weight: flex.shrink * width.clamp(),
            },
            FlexDirection::Column => Self {
                element,
                base_size: FlexValue::new(height, width),
                final_size: FlexValue::new(0.0, 0.0),
                margin: FlexValue::new(
                    FlexBlock::new(margin.top, margin.bottom),
                    FlexBlock::new(margin.left, margin.right),
                ),
                length: FlexValue::new(style.height.value, style.width.value),
                grow: flex.grow,
                shrink: flex.shrink,
                weight: flex.shrink * height.clamp(),
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

pub struct FlexCursor {
    start: FlexValue<f32>,
    offset: FlexValue<f32>,
    spacing: FlexValue<f32>,
}

impl FlexCursor {
    pub fn new(
        style: &ComputedStyle,
        parent: &Rect,
        axis: &FlexValue<FlexAxis>,
        lines: &[FlexLine],
    ) -> Self {
        let gap_space = (lines.len().saturating_sub(1)) as f32 * axis.cross.gap;
        let used_space = lines.iter().map(|l| l.size.cross).sum::<f32>() + gap_space;
        let free_space = axis.cross.space - used_space;

        let (main_start, cross_start) = match style.flex.direction {
            FlexDirection::Row => (parent.x, parent.y),
            FlexDirection::Column => (parent.y, parent.x),
        };

        let (cross_offset, cross_spacing) = match style.align {
            Align::Start => (0.0, axis.cross.gap),
            Align::Center => ((free_space * 0.5).max(0.0), axis.cross.gap),
            Align::End => (free_space.max(0.0), axis.cross.gap),
            Align::Stretch => (0.0, axis.cross.gap),
        };

        Self {
            start: FlexValue::new(main_start, cross_start),
            offset: FlexValue::new(0.0, cross_offset),
            spacing: FlexValue::new(0.0, cross_spacing),
        }
    }

    pub fn start(
        &mut self,
        justify: Justify,
        parent: &Rect,
        axis: &FlexValue<FlexAxis>,
        line: &FlexLine,
    ) {
        let free_space = axis.main.space - line.size.main;

        let (offset, spacing) = match justify {
            Justify::Start => (0.0, axis.main.gap),
            Justify::Center => ((free_space * 0.5).max(0.0), axis.main.gap),
            Justify::End => (free_space.max(0.0), axis.main.gap),
            Justify::Between => {
                let gap = if line.items.len() > 1 {
                    axis.main.gap + free_space.max(0.0) / (line.items.len() - 1) as f32
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

                (slot * 0.5, axis.main.gap + slot)
            }
            Justify::Evenly => {
                let slot = free_space.max(0.0) / (line.items.len() + 1) as f32;
                (slot, axis.main.gap + slot)
            }
        };

        self.offset.main = self.start.main + offset;
        self.spacing.main = spacing;
    }

    pub fn next(&mut self, item: &FlexItem) {
        self.offset.main += self.spacing.main + item.margin.main.size() + item.final_size.main;
    }

    pub fn end(&mut self, size: f32) {
        self.offset.cross += self.spacing.cross + size;
    }
}
