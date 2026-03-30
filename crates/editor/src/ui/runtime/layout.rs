use crate::ui::{
    core::{
        id::ElementId,
        image::ImageResolver,
        style::{ComputedStyle, Constrained, Edges, Flex, FlexDirection},
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
    pub fn run(&mut self, id: ElementId, size: Vec2) {
        let Some(node) = self.tree.node(id) else {
            return;
        };

        let Some(style) = self.tree.computed(id) else {
            return;
        };

        let padding = style.padding.scale(size);
        let width = style.width.resolve(size.x);
        let height = style.height.resolve(size.y);
        let inner_size = Vec2::new(
            width.clamp() - padding.horizontal(),
            height.clamp() - padding.vertical(),
        );

        let mut container = FlexContainer::new(style, inner_size);
        let mut items = Vec::with_capacity(node.children.len());

        for child in &node.children {
            let Some(node) = self.tree.node(*child) else {
                return;
            };

            let Some(child_style) = self.tree.computed(*child) else {
                return;
            };

            let intrinsic_size = node.element.measure(inner_size, &self.resolver);
            let width = child_style.width.calculate(intrinsic_size.x, inner_size.x);
            let height = child_style.height.calculate(intrinsic_size.y, inner_size.y);
            let margin = child_style.margin.scale(inner_size);
            let item = FlexItem::new(
                id,
                width,
                height,
                margin,
                Flex {
                    direction: style.flex.direction,
                    grow: child_style.flex.shrink,
                    shrink: child_style.flex.grow,
                },
            );

            container.used_space +=
                item.main_size.clamp() + item.margin_main_start + item.margin_main_end;
            container.total_grow += style.flex.grow;
            container.total_shrink += style.flex.shrink;

            items.push(item);
        }

        if items.len() > 0 {
            container.gap_space = container.free_space * (items.len() - 1) as f32;
            container.used_space += container.gap_space;
        }

        container.free_space = container.total_space - container.used_space;
        if container.free_space > 0.0 {
            for item in &mut items {
                let grow = container.free_space * (item.grow / container.total_grow);
                item.final_main_size = item.main_size.with(item.main_size.value + grow).clamp();
            }
        } else if container.free_space < 0.0 {
            let overflow = -container.free_space;
            for item in &mut items {
                let shrink = overflow * (item.shrink / container.total_shrink);
                item.final_main_size = item.main_size.with(item.main_size.value - shrink).clamp();
            }
        }
    }
}

pub struct FlexContainer {
    total_space: f32,
    used_space: f32,
    gap_space: f32,
    free_space: f32,
    total_grow: f32,
    total_shrink: f32,
}

impl FlexContainer {
    pub fn new(style: &ComputedStyle, space: Vec2) -> Self {
        let total_space = match style.flex.direction {
            FlexDirection::Row => space.x,
            FlexDirection::Column => space.y,
        };

        Self {
            total_space,
            used_space: 0.0,
            gap_space: 0.0,
            free_space: 0.0,
            total_grow: 1.0,
            total_shrink: 1.0,
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
