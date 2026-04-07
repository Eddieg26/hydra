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
use std::ops::Range;

pub struct ContentResolver<'a> {
    pub text: &'a mut dyn TextMeasurer,
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

        let axis = FlexAxis::pair(style, content);

        let mut items = self.create_items(node, style, &content);
        let mut lines = self.create_lines(style.wrap, &axis.main, &mut items);

        self.pack_main_axis(&axis.main, &mut items, &mut lines);
        self.pack_cross_axis(style.align, &mut items, &mut lines);

        self.tree
            .layouts
            .insert(id, Layout::new(rect, content, clip, border_width));

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
            cursor.start(style.justify, axis, &line);

            for index in line.items {
                let item = &items[index];
                let rect = match style.flex.direction {
                    FlexDirection::Row => Rect {
                        x: (cursor.offset.main + item.margin.main.start).round(),
                        y: (cursor.offset.cross + item.margin.cross.start).round(),
                        width: item.final_size.main,
                        height: item.final_size.cross,
                    },
                    FlexDirection::Column => Rect {
                        x: (cursor.offset.cross + item.margin.cross.start).round(),
                        y: (cursor.offset.main + item.margin.main.start).round(),
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
                let area = content.intersection(&parent);
                (area.x, area.width)
            }
        };

        let (y, height) = match style.overlfow_y {
            Overflow::Visible => (parent.y, parent.height),
            Overflow::Hidden | Overflow::Scroll => {
                let area = content.intersection(&parent);
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

    pub fn start(&mut self, justify: Justify, axis: &FlexValue<FlexAxis>, line: &FlexLine) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::{
        core::{
            id::ElementId,
            image::{ImageHandle, ImageResolver},
            style::*,
            text::{TextMeasurement, TextMeasurer},
        },
        runtime::{
            node::{Element, ElementFlags, ElementNode},
            tree::ElementTree,
        },
    };
    use math::{Size, Vec2, rect::Rect};
    use renderer::types::Color;
    use slotmap::{SecondaryMap, SlotMap};

    // ── Helpers ──────────────────────────────────────────────────────

    const TRANSPARENT: Color = Color {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 0.0,
    };

    fn default_computed_style() -> ComputedStyle {
        ComputedStyle {
            display: Display::Flex,
            position: Position::Relative,
            visibility: Visibility::Visible,
            width: Constrained {
                value: Length::Auto,
                min: Length::Fixed(0.0),
                max: Length::Fixed(f32::INFINITY),
            },
            height: Constrained {
                value: Length::Auto,
                min: Length::Fixed(0.0),
                max: Length::Fixed(f32::INFINITY),
            },
            background: TRANSPARENT,
            border: None,
            text: TextStyle {
                font: None,
                color: TRANSPARENT,
                font_size: 16.0,
                line_height: 20.0,
                align: TextAlign::Left,
                wrap_width: None,
                weight: FontWeight::Regular,
            },
            flex: Flex {
                direction: FlexDirection::Row,
                grow: 0.0,
                shrink: 0.0,
            },
            wrap: FlexWrap::None,
            justify: Justify::Start,
            align: Align::Start,
            margin: Edges {
                left: 0.0,
                right: 0.0,
                top: 0.0,
                bottom: 0.0,
            },
            padding: Edges {
                left: 0.0,
                right: 0.0,
                top: 0.0,
                bottom: 0.0,
            },
            overflow_x: Overflow::Visible,
            overlfow_y: Overflow::Visible,
            gap_x: Length::Fixed(0.0),
            gap_y: Length::Fixed(0.0),
            opacity: 1.0,
        }
    }

    /// A simple box element with a fixed intrinsic size.
    struct BoxElement {
        size: Size,
    }

    impl BoxElement {
        fn new(w: f32, h: f32) -> Self {
            Self {
                size: Size::new(w, h),
            }
        }
    }

    impl Element for BoxElement {
        fn measure(&self, _resolver: &ContentResolver) -> Option<Size> {
            Some(self.size)
        }

        fn draw(
            &self,
            _style: &ComputedStyle,
            _layout: &Layout,
        ) -> crate::ui::runtime::paint::DrawCommand {
            crate::ui::runtime::paint::DrawCommand::Quad {
                rect: Rect::ZERO,
                color: TRANSPARENT,
            }
        }
    }

    struct DummyText;
    impl TextMeasurer for DummyText {}

    struct DummyImage;
    impl ImageResolver for DummyImage {
        fn size(&self, _image: ImageHandle) -> Vec2 {
            Vec2::ZERO
        }
    }

    fn make_resolver() -> (DummyText, DummyImage) {
        (DummyText, DummyImage)
    }

    /// Build a tree with a root and N children. Returns (tree, root_id, child_ids).
    fn build_tree(
        root_style: ComputedStyle,
        children: Vec<(ComputedStyle, Box<dyn Element>)>,
    ) -> (ElementTree, ElementId, Vec<ElementId>) {
        let mut nodes: SlotMap<ElementId, ElementNode> = SlotMap::with_key();
        let mut element_styles: SecondaryMap<ElementId, ElementStyle> = SecondaryMap::new();
        let mut computed_styles: SecondaryMap<ElementId, ComputedStyle> = SecondaryMap::new();
        let layouts: SecondaryMap<ElementId, Layout> = SecondaryMap::new();

        let root_id = nodes.insert(ElementNode {
            id: ElementId::default(),
            parent: None,
            children: Vec::new(),
            element: Box::new(BoxElement::new(0.0, 0.0)),
            flags: ElementFlags::empty(),
        });
        nodes[root_id].id = root_id;
        computed_styles.insert(root_id, root_style);
        element_styles.insert(
            root_id,
            ElementStyle {
                inline: Style {
                    display: None,
                    position: None,
                    visibility: None,
                    width: None,
                    height: None,
                    min_width: None,
                    min_height: None,
                    max_width: None,
                    max_height: None,
                    background: None,
                    border: None,
                    text: None,
                    flex_direction: None,
                    flex_grow: None,
                    flex_shrink: None,
                    justify: None,
                    wrap: None,
                    align: None,
                    gap_x: None,
                    gap_y: None,
                    margin: None,
                    padding: None,
                    overflow_x: None,
                    overlfow_y: None,
                    opacity: None,
                },
                classes: smallvec::smallvec![],
            },
        );

        let mut child_ids = Vec::new();
        for (child_style, element) in children {
            let child_id = nodes.insert(ElementNode {
                id: ElementId::default(),
                parent: Some(root_id),
                children: Vec::new(),
                element,
                flags: ElementFlags::empty(),
            });
            nodes[child_id].id = child_id;
            computed_styles.insert(child_id, child_style);
            element_styles.insert(
                child_id,
                ElementStyle {
                    inline: Style {
                        display: None,
                        position: None,
                        visibility: None,
                        width: None,
                        height: None,
                        min_width: None,
                        min_height: None,
                        max_width: None,
                        max_height: None,
                        background: None,
                        border: None,
                        text: None,
                        flex_direction: None,
                        flex_grow: None,
                        flex_shrink: None,
                        justify: None,
                        wrap: None,
                        align: None,
                        gap_x: None,
                        gap_y: None,
                        margin: None,
                        padding: None,
                        overflow_x: None,
                        overlfow_y: None,
                        opacity: None,
                    },
                    classes: smallvec::smallvec![],
                },
            );
            nodes[root_id].children.push(child_id);
            child_ids.push(child_id);
        }

        let tree = ElementTree {
            nodes,
            element_styles,
            computed_styles,
            layouts,
        };

        (tree, root_id, child_ids)
    }

    // ── FlexBlock tests ─────────────────────────────────────────────

    #[test]
    fn flex_block_size() {
        let block = FlexBlock::new(10.0, 20.0);
        assert_eq!(block.size(), 30.0);
    }

    #[test]
    fn flex_block_clone() {
        let block = FlexBlock::new(5.0, 7.0);
        let cloned = block.clone();
        assert_eq!(cloned.start, 5.0);
        assert_eq!(cloned.end, 7.0);
    }

    // ── FlexValue tests ─────────────────────────────────────────────

    #[test]
    fn flex_value_new_and_clone() {
        let val = FlexValue::new(1.0_f32, 2.0_f32);
        assert_eq!(val.main, 1.0);
        assert_eq!(val.cross, 2.0);
        let cloned = val.clone();
        assert_eq!(cloned.main, 1.0);
        assert_eq!(cloned.cross, 2.0);
    }

    // ── FlexLine tests ──────────────────────────────────────────────

    #[test]
    fn flex_line_new() {
        let line = FlexLine::new(3);
        assert_eq!(line.items, 3..3);
        assert_eq!(line.grow, 0.0);
        assert_eq!(line.shrink, 0.0);
    }

    #[test]
    fn flex_line_default_starts_at_zero() {
        let line = FlexLine::default();
        assert_eq!(line.items, 0..0);
    }

    // ── FlexAxis::pair tests ────────────────────────────────────────

    #[test]
    fn flex_axis_pair_row() {
        let mut style = default_computed_style();
        style.flex.direction = FlexDirection::Row;
        style.gap_x = Length::Fixed(5.0);
        style.gap_y = Length::Fixed(10.0);

        let rect = Rect::new(0.0, 0.0, 200.0, 100.0);
        let pair = FlexAxis::pair(&style, rect);

        assert_eq!(pair.main.space, 200.0);
        assert_eq!(pair.main.gap, 5.0);
        assert_eq!(pair.cross.space, 100.0);
        assert_eq!(pair.cross.gap, 10.0);
    }

    #[test]
    fn flex_axis_pair_column() {
        let mut style = default_computed_style();
        style.flex.direction = FlexDirection::Column;
        style.gap_x = Length::Fixed(5.0);
        style.gap_y = Length::Fixed(10.0);

        let rect = Rect::new(0.0, 0.0, 200.0, 100.0);
        let pair = FlexAxis::pair(&style, rect);

        // Column: main = y-axis, cross = x-axis
        assert_eq!(pair.main.space, 100.0);
        assert_eq!(pair.main.gap, 10.0);
        assert_eq!(pair.cross.space, 200.0);
        assert_eq!(pair.cross.gap, 5.0);
    }

    // ── FlexItem::new tests ─────────────────────────────────────────

    #[test]
    fn flex_item_row_maps_width_to_main() {
        let style = default_computed_style();
        let width = Constrained {
            value: 50.0,
            min: 0.0,
            max: f32::INFINITY,
        };
        let height = Constrained {
            value: 30.0,
            min: 0.0,
            max: f32::INFINITY,
        };
        let margin = Edges {
            left: 1.0,
            right: 2.0,
            top: 3.0,
            bottom: 4.0,
        };
        let flex = Flex {
            direction: FlexDirection::Row,
            grow: 1.0,
            shrink: 0.5,
        };

        let item = FlexItem::new(ElementId::default(), &style, width, height, margin, flex);

        assert_eq!(item.base_size.main.value, 50.0);
        assert_eq!(item.base_size.cross.value, 30.0);
        assert_eq!(item.margin.main.start, 1.0); // left
        assert_eq!(item.margin.main.end, 2.0); // right
        assert_eq!(item.margin.cross.start, 3.0); // top
        assert_eq!(item.margin.cross.end, 4.0); // bottom
        assert_eq!(item.grow, 1.0);
        assert_eq!(item.shrink, 0.5);
    }

    #[test]
    fn flex_item_column_maps_height_to_main() {
        let style = default_computed_style();
        let width = Constrained {
            value: 50.0,
            min: 0.0,
            max: f32::INFINITY,
        };
        let height = Constrained {
            value: 30.0,
            min: 0.0,
            max: f32::INFINITY,
        };
        let margin = Edges {
            left: 1.0,
            right: 2.0,
            top: 3.0,
            bottom: 4.0,
        };
        let flex = Flex {
            direction: FlexDirection::Column,
            grow: 2.0,
            shrink: 1.0,
        };

        let item = FlexItem::new(ElementId::default(), &style, width, height, margin, flex);

        assert_eq!(item.base_size.main.value, 30.0); // height is main
        assert_eq!(item.base_size.cross.value, 50.0); // width is cross
        assert_eq!(item.margin.main.start, 3.0); // top
        assert_eq!(item.margin.main.end, 4.0); // bottom
        assert_eq!(item.margin.cross.start, 1.0); // left
        assert_eq!(item.margin.cross.end, 2.0); // right
    }

    // ── Layout::new tests ───────────────────────────────────────────

    #[test]
    fn layout_new_stores_fields() {
        let outer = Rect::new(10.0, 20.0, 100.0, 50.0);
        let content = Rect::new(12.0, 22.0, 96.0, 46.0);
        let clip = Rect::new(12.0, 22.0, 96.0, 46.0);
        let layout = Layout::new(outer, content, clip, 2.0);

        assert_eq!(layout.outer, outer);
        assert_eq!(layout.content, content);
        assert_eq!(layout.clip, clip);
        assert_eq!(layout.border.width, 2.0);
        assert_eq!(layout.scroll, Rect::ZERO);
    }

    // ── Layout::clip tests ──────────────────────────────────────────

    #[test]
    fn clip_visible_uses_parent_bounds() {
        let mut style = default_computed_style();
        style.overflow_x = Overflow::Visible;
        style.overlfow_y = Overflow::Visible;

        let content = Rect::new(10.0, 10.0, 80.0, 60.0);
        let parent = Rect::new(0.0, 0.0, 200.0, 200.0);

        let clip = Layout::clip(&style, &content, Some(&parent));
        assert_eq!(clip.x, 0.0);
        assert_eq!(clip.y, 0.0);
        assert_eq!(clip.width, 200.0);
        assert_eq!(clip.height, 200.0);
    }

    #[test]
    fn clip_hidden_intersects_with_parent() {
        let mut style = default_computed_style();
        style.overflow_x = Overflow::Hidden;
        style.overlfow_y = Overflow::Hidden;

        let content = Rect::new(10.0, 10.0, 80.0, 60.0);
        let parent = Rect::new(0.0, 0.0, 50.0, 50.0);

        let clip = Layout::clip(&style, &content, Some(&parent));
        // intersect of content(10,10,80,60) and parent(0,0,50,50)
        assert_eq!(clip.x, 10.0);
        assert_eq!(clip.y, 10.0);
        assert_eq!(clip.width, 40.0); // min(90,50) - 10
        assert_eq!(clip.height, 40.0); // min(70,50) - 10
    }

    #[test]
    fn clip_no_parent_uses_content_as_parent() {
        let mut style = default_computed_style();
        style.overflow_x = Overflow::Visible;
        style.overlfow_y = Overflow::Visible;

        let content = Rect::new(5.0, 5.0, 100.0, 80.0);
        let clip = Layout::clip(&style, &content, None);

        assert_eq!(clip.x, 5.0);
        assert_eq!(clip.width, 100.0);
    }

    // ── FlexCursor tests ────────────────────────────────────────────

    fn make_line(items: Range<usize>, main: f32, cross: f32) -> FlexLine {
        FlexLine {
            items,
            size: FlexValue::new(main, cross),
            grow: 0.0,
            shrink: 0.0,
            weight: 0.0,
        }
    }

    #[test]
    fn cursor_justify_start() {
        let style = default_computed_style();
        let parent = Rect::new(0.0, 0.0, 300.0, 100.0);
        let axis = FlexAxis::pair(&style, parent);

        let lines = [make_line(0..2, 100.0, 30.0)];
        let mut cursor = FlexCursor::new(&style, &parent, &axis, &lines);
        let line = make_line(0..2, 100.0, 30.0);
        cursor.start(Justify::Start, &axis, &line);

        assert_eq!(cursor.offset.main, 0.0);
        assert_eq!(cursor.spacing.main, 0.0);
    }

    #[test]
    fn cursor_justify_center() {
        let style = default_computed_style();
        let parent = Rect::new(0.0, 0.0, 300.0, 100.0);
        let axis = FlexAxis::pair(&style, parent);

        let lines = [make_line(0..2, 100.0, 30.0)];
        let mut cursor = FlexCursor::new(&style, &parent, &axis, &lines);
        let line = make_line(0..2, 100.0, 30.0);
        cursor.start(Justify::Center, &axis, &line);

        // free_space = 300 - 100 = 200, offset = 100
        assert_eq!(cursor.offset.main, 100.0);
    }

    #[test]
    fn cursor_justify_end() {
        let style = default_computed_style();
        let parent = Rect::new(0.0, 0.0, 300.0, 100.0);
        let axis = FlexAxis::pair(&style, parent);

        let lines = [make_line(0..2, 100.0, 30.0)];
        let mut cursor = FlexCursor::new(&style, &parent, &axis, &lines);
        let line = make_line(0..2, 100.0, 30.0);
        cursor.start(Justify::End, &axis, &line);

        assert_eq!(cursor.offset.main, 200.0);
    }

    #[test]
    fn cursor_justify_between() {
        let mut style = default_computed_style();
        style.gap_x = Length::Fixed(0.0);
        let parent = Rect::new(0.0, 0.0, 300.0, 100.0);
        let axis = FlexAxis::pair(&style, parent);

        let lines = [make_line(0..3, 90.0, 30.0)];
        let mut cursor = FlexCursor::new(&style, &parent, &axis, &lines);
        let line = make_line(0..3, 90.0, 30.0);
        cursor.start(Justify::Between, &axis, &line);

        // free_space = 210, gap = 210 / 2 = 105
        assert_eq!(cursor.offset.main, 0.0);
        assert_eq!(cursor.spacing.main, 105.0);
    }

    #[test]
    fn cursor_justify_around() {
        let mut style = default_computed_style();
        style.gap_x = Length::Fixed(0.0);
        let parent = Rect::new(0.0, 0.0, 300.0, 100.0);
        let axis = FlexAxis::pair(&style, parent);

        let lines = [make_line(0..3, 90.0, 30.0)];
        let mut cursor = FlexCursor::new(&style, &parent, &axis, &lines);
        let line = make_line(0..3, 90.0, 30.0);
        cursor.start(Justify::Around, &axis, &line);

        // free_space = 210, slot = 70, offset = 35, spacing = 70
        assert_eq!(cursor.offset.main, 35.0);
        assert_eq!(cursor.spacing.main, 70.0);
    }

    #[test]
    fn cursor_justify_evenly() {
        let mut style = default_computed_style();
        style.gap_x = Length::Fixed(0.0);
        let parent = Rect::new(0.0, 0.0, 300.0, 100.0);
        let axis = FlexAxis::pair(&style, parent);

        let lines = [make_line(0..3, 90.0, 30.0)];
        let mut cursor = FlexCursor::new(&style, &parent, &axis, &lines);
        let line = make_line(0..3, 90.0, 30.0);
        cursor.start(Justify::Evenly, &axis, &line);

        // free_space = 210, slot = 52.5
        assert_eq!(cursor.offset.main, 52.5);
        assert_eq!(cursor.spacing.main, 52.5);
    }

    #[test]
    fn cursor_next_advances_offset() {
        let style = default_computed_style();
        let parent = Rect::new(0.0, 0.0, 300.0, 100.0);
        let axis = FlexAxis::pair(&style, parent);

        let lines = [make_line(0..1, 50.0, 30.0)];
        let mut cursor = FlexCursor::new(&style, &parent, &axis, &lines);
        let line = make_line(0..1, 50.0, 30.0);
        cursor.start(Justify::Start, &axis, &line);

        let item = FlexItem {
            element: ElementId::default(),
            base_size: FlexValue::new(
                Constrained {
                    value: 40.0,
                    min: 0.0,
                    max: f32::INFINITY,
                },
                Constrained {
                    value: 30.0,
                    min: 0.0,
                    max: f32::INFINITY,
                },
            ),
            final_size: FlexValue::new(40.0, 30.0),
            margin: FlexValue::new(FlexBlock::new(5.0, 5.0), FlexBlock::new(0.0, 0.0)),
            length: FlexValue::new(Length::Fixed(40.0), Length::Auto),
            grow: 0.0,
            shrink: 0.0,
            weight: 0.0,
        };

        cursor.next(&item);
        // offset += spacing(0) + margin.main.size(10) + final_size.main(40) = 50
        assert_eq!(cursor.offset.main, 50.0);
    }

    #[test]
    fn cursor_end_advances_cross() {
        let style = default_computed_style();
        let parent = Rect::new(0.0, 0.0, 300.0, 100.0);
        let axis = FlexAxis::pair(&style, parent);

        let lines = [make_line(0..1, 50.0, 30.0)];
        let mut cursor = FlexCursor::new(&style, &parent, &axis, &lines);
        let initial_cross = cursor.offset.cross;
        cursor.end(30.0);
        assert_eq!(cursor.offset.cross, initial_cross + 30.0);
    }

    // ── LayoutEngine integration tests ──────────────────────────────

    fn run_layout(
        root_style: ComputedStyle,
        children: Vec<(ComputedStyle, Box<dyn Element>)>,
        viewport: Rect,
    ) -> (ElementTree, ElementId, Vec<ElementId>) {
        let (mut tree, root_id, child_ids) = build_tree(root_style, children);
        let (mut text, image) = make_resolver();
        let resolver = ContentResolver {
            text: &mut text,
            image: &image,
        };
        let mut engine = LayoutEngine {
            tree: &mut tree,
            resolver,
        };
        engine.run(root_id, viewport);
        (tree, root_id, child_ids)
    }

    #[test]
    fn layout_single_child_row() {
        let root_style = default_computed_style();
        let child_style = default_computed_style();

        let (tree, root_id, child_ids) = run_layout(
            root_style,
            vec![(child_style, Box::new(BoxElement::new(50.0, 30.0)))],
            Rect::new(0.0, 0.0, 200.0, 100.0),
        );

        let root_layout = tree.layout(root_id).unwrap();
        assert_eq!(root_layout.outer, Rect::new(0.0, 0.0, 200.0, 100.0));

        let child_layout = tree.layout(child_ids[0]).unwrap();
        assert_eq!(child_layout.outer.x, 0.0);
        assert_eq!(child_layout.outer.y, 0.0);
        assert_eq!(child_layout.outer.width, 50.0);
        assert_eq!(child_layout.outer.height, 30.0);
    }

    #[test]
    fn layout_two_children_row() {
        let root_style = default_computed_style();
        let child_style = default_computed_style();

        let (tree, _root_id, child_ids) = run_layout(
            root_style,
            vec![
                (child_style, Box::new(BoxElement::new(60.0, 40.0))),
                (child_style, Box::new(BoxElement::new(80.0, 50.0))),
            ],
            Rect::new(0.0, 0.0, 300.0, 100.0),
        );

        let first = tree.layout(child_ids[0]).unwrap();
        let second = tree.layout(child_ids[1]).unwrap();

        // Row layout: second child starts after first
        assert_eq!(first.outer.x, 0.0);
        assert_eq!(second.outer.x, 60.0);
        assert_eq!(first.outer.width, 60.0);
        assert_eq!(second.outer.width, 80.0);
    }

    #[test]
    fn layout_column_direction() {
        let mut root_style = default_computed_style();
        root_style.flex.direction = FlexDirection::Column;
        let child_style = default_computed_style();

        let (tree, _root_id, child_ids) = run_layout(
            root_style,
            vec![
                (child_style, Box::new(BoxElement::new(60.0, 40.0))),
                (child_style, Box::new(BoxElement::new(80.0, 50.0))),
            ],
            Rect::new(0.0, 0.0, 300.0, 200.0),
        );

        let first = tree.layout(child_ids[0]).unwrap();
        let second = tree.layout(child_ids[1]).unwrap();

        // Column: items stack vertically
        assert_eq!(first.outer.y, 0.0);
        assert_eq!(second.outer.y, 40.0);
        assert_eq!(first.outer.height, 40.0);
        assert_eq!(second.outer.height, 50.0);
    }

    #[test]
    fn layout_with_padding() {
        let mut root_style = default_computed_style();
        root_style.padding = Edges {
            left: 10.0,
            right: 10.0,
            top: 5.0,
            bottom: 5.0,
        };
        let child_style = default_computed_style();

        let (tree, root_id, child_ids) = run_layout(
            root_style,
            vec![(child_style, Box::new(BoxElement::new(50.0, 30.0)))],
            Rect::new(0.0, 0.0, 200.0, 100.0),
        );

        let root_layout = tree.layout(root_id).unwrap();
        // Content rect accounts for padding
        assert_eq!(root_layout.content.x, 0.0);
        assert_eq!(root_layout.content.y, 0.0);
        assert_eq!(root_layout.content.width, 180.0); // 200 - 10 - 10
        assert_eq!(root_layout.content.height, 90.0); // 100 - 5 - 5
    }

    #[test]
    fn layout_with_border() {
        let mut root_style = default_computed_style();
        root_style.border = Some(Border {
            width: 2.0,
            color: Edges {
                left: TRANSPARENT,
                right: TRANSPARENT,
                top: TRANSPARENT,
                bottom: TRANSPARENT,
            },
        });
        let child_style = default_computed_style();

        let (tree, root_id, child_ids) = run_layout(
            root_style,
            vec![(child_style, Box::new(BoxElement::new(50.0, 30.0)))],
            Rect::new(0.0, 0.0, 200.0, 100.0),
        );

        let root_layout = tree.layout(root_id).unwrap();
        assert_eq!(root_layout.border.width, 2.0);
        // Content starts offset by border
        assert_eq!(root_layout.content.x, 2.0);
        assert_eq!(root_layout.content.y, 2.0);
    }

    #[test]
    fn layout_flex_grow() {
        let root_style = default_computed_style();
        let mut child1 = default_computed_style();
        child1.flex.grow = 1.0;
        let mut child2 = default_computed_style();
        child2.flex.grow = 1.0;

        let (tree, _root_id, child_ids) = run_layout(
            root_style,
            vec![
                (child1, Box::new(BoxElement::new(0.0, 30.0))),
                (child2, Box::new(BoxElement::new(0.0, 30.0))),
            ],
            Rect::new(0.0, 0.0, 200.0, 100.0),
        );

        let first = tree.layout(child_ids[0]).unwrap();
        let second = tree.layout(child_ids[1]).unwrap();

        // Both grow equally to fill 200px
        assert_eq!(first.outer.width, 100.0);
        assert_eq!(second.outer.width, 100.0);
        assert_eq!(second.outer.x, 100.0);
    }

    #[test]
    fn layout_flex_grow_unequal() {
        let root_style = default_computed_style();
        let mut child1 = default_computed_style();
        child1.flex.grow = 1.0;
        let mut child2 = default_computed_style();
        child2.flex.grow = 3.0;

        let (tree, _root_id, child_ids) = run_layout(
            root_style,
            vec![
                (child1, Box::new(BoxElement::new(0.0, 30.0))),
                (child2, Box::new(BoxElement::new(0.0, 30.0))),
            ],
            Rect::new(0.0, 0.0, 200.0, 100.0),
        );

        let first = tree.layout(child_ids[0]).unwrap();
        let second = tree.layout(child_ids[1]).unwrap();

        assert_eq!(first.outer.width, 50.0);
        assert_eq!(second.outer.width, 150.0);
    }

    #[test]
    fn layout_with_gap() {
        let mut root_style = default_computed_style();
        root_style.gap_x = Length::Fixed(10.0);
        let child_style = default_computed_style();

        let (tree, _root_id, child_ids) = run_layout(
            root_style,
            vec![
                (child_style, Box::new(BoxElement::new(40.0, 30.0))),
                (child_style, Box::new(BoxElement::new(40.0, 30.0))),
                (child_style, Box::new(BoxElement::new(40.0, 30.0))),
            ],
            Rect::new(0.0, 0.0, 300.0, 100.0),
        );

        let c0 = tree.layout(child_ids[0]).unwrap();
        let c1 = tree.layout(child_ids[1]).unwrap();
        let c2 = tree.layout(child_ids[2]).unwrap();

        assert_eq!(c0.outer.x, 0.0);
        assert_eq!(c1.outer.x, 50.0); // 40 + 10 gap
        assert_eq!(c2.outer.x, 100.0); // 50 + 40 + 10 gap
    }

    #[test]
    fn layout_with_child_margin() {
        let root_style = default_computed_style();
        let mut child_style = default_computed_style();
        child_style.margin = Edges {
            left: 10.0,
            right: 10.0,
            top: 5.0,
            bottom: 5.0,
        };

        let (tree, _root_id, child_ids) = run_layout(
            root_style,
            vec![(child_style, Box::new(BoxElement::new(50.0, 30.0)))],
            Rect::new(0.0, 0.0, 200.0, 100.0),
        );

        let child = tree.layout(child_ids[0]).unwrap();
        // Child rect offset by margin
        assert_eq!(child.outer.x, 10.0);
        assert_eq!(child.outer.y, 5.0);
        assert_eq!(child.outer.width, 50.0);
        assert_eq!(child.outer.height, 30.0);
    }

    #[test]
    fn layout_no_children() {
        let root_style = default_computed_style();

        let (tree, root_id, _) = run_layout(root_style, vec![], Rect::new(0.0, 0.0, 200.0, 100.0));

        let root_layout = tree.layout(root_id).unwrap();
        assert_eq!(root_layout.outer, Rect::new(0.0, 0.0, 200.0, 100.0));
        assert_eq!(root_layout.scroll, Rect::ZERO);
    }

    #[test]
    fn layout_justify_center_integration() {
        let mut root_style = default_computed_style();
        root_style.justify = Justify::Center;
        let child_style = default_computed_style();

        let (tree, _root_id, child_ids) = run_layout(
            root_style,
            vec![(child_style, Box::new(BoxElement::new(100.0, 30.0)))],
            Rect::new(0.0, 0.0, 300.0, 100.0),
        );

        let child = tree.layout(child_ids[0]).unwrap();
        // Centered: (300 - 100) / 2 = 100
        assert_eq!(child.outer.x, 100.0);
    }

    #[test]
    fn layout_justify_end_integration() {
        let mut root_style = default_computed_style();
        root_style.justify = Justify::End;
        let child_style = default_computed_style();

        let (tree, _root_id, child_ids) = run_layout(
            root_style,
            vec![(child_style, Box::new(BoxElement::new(100.0, 30.0)))],
            Rect::new(0.0, 0.0, 300.0, 100.0),
        );

        let child = tree.layout(child_ids[0]).unwrap();
        assert_eq!(child.outer.x, 200.0);
    }

    #[test]
    fn layout_align_stretch_with_taller_sibling() {
        let mut root_style = default_computed_style();
        root_style.align = Align::Stretch;
        let child_style = default_computed_style();

        let (tree, _root_id, child_ids) = run_layout(
            root_style,
            vec![
                (child_style, Box::new(BoxElement::new(50.0, 0.0))),
                (child_style, Box::new(BoxElement::new(50.0, 80.0))),
            ],
            Rect::new(0.0, 0.0, 200.0, 100.0),
        );

        let first = tree.layout(child_ids[0]).unwrap();
        let second = tree.layout(child_ids[1]).unwrap();

        // Line cross size = max(0, 80) = 80
        // First child (Auto height) stretches to 80
        assert_eq!(first.outer.height, 80.0);
        assert_eq!(second.outer.height, 80.0);
    }

    #[test]
    fn layout_wrap_creates_multiple_lines() {
        let mut root_style = default_computed_style();
        root_style.wrap = FlexWrap::Wrap;
        let child_style = default_computed_style();

        let (tree, _root_id, child_ids) = run_layout(
            root_style,
            vec![
                (child_style, Box::new(BoxElement::new(60.0, 30.0))),
                (child_style, Box::new(BoxElement::new(60.0, 30.0))),
                (child_style, Box::new(BoxElement::new(60.0, 30.0))),
            ],
            Rect::new(0.0, 0.0, 100.0, 200.0),
        );

        let c0 = tree.layout(child_ids[0]).unwrap();
        let c1 = tree.layout(child_ids[1]).unwrap();
        let c2 = tree.layout(child_ids[2]).unwrap();

        // First line: item 0 (60px), can't fit item 1 (60+60=120 > 100)
        assert_eq!(c0.outer.x, 0.0);
        assert_eq!(c0.outer.y, 0.0);

        // Second line: item 1
        assert_eq!(c1.outer.x, 0.0);
        assert_eq!(c1.outer.y, 30.0);

        // Third line: item 2
        assert_eq!(c2.outer.x, 0.0);
        assert_eq!(c2.outer.y, 60.0);
    }

    #[test]
    fn layout_fixed_size_child() {
        let root_style = default_computed_style();
        let mut child_style = default_computed_style();
        child_style.width = Constrained {
            value: Length::Fixed(80.0),
            min: Length::Fixed(0.0),
            max: Length::Fixed(f32::INFINITY),
        };
        child_style.height = Constrained {
            value: Length::Fixed(40.0),
            min: Length::Fixed(0.0),
            max: Length::Fixed(f32::INFINITY),
        };

        let (tree, _root_id, child_ids) = run_layout(
            root_style,
            vec![(child_style, Box::new(BoxElement::new(10.0, 10.0)))],
            Rect::new(0.0, 0.0, 200.0, 100.0),
        );

        let child = tree.layout(child_ids[0]).unwrap();
        // Fixed size overrides intrinsic
        assert_eq!(child.outer.width, 80.0);
        assert_eq!(child.outer.height, 40.0);
    }
}
