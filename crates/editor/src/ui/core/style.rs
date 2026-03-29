use renderer::types::Color;
use smallvec::SmallVec;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy)]
pub struct Edges<T> {
    pub left: T,
    pub right: T,
    pub top: T,
    pub bottom: T,
}

#[derive(Debug, Clone, Copy)]
pub enum Length {
    Auto,
    Fixed(f32),
    Fill(f32),
}

#[derive(Debug, Clone, Copy)]
pub enum Display {
    Flex,
    None,
}

#[derive(Debug, Clone, Copy)]
pub enum FlexDirection {
    Row,
    Column,
}

#[derive(Debug, Clone, Copy)]
pub enum Position {
    Relative,
    Absolute,
}

#[derive(Debug, Clone, Copy)]
pub enum Overflow {
    Visible,
    Hidden,
    Scroll,
}

#[derive(Debug, Clone, Copy)]
pub enum Visibility {
    Visible,
    Hidden,
}

#[derive(Debug, Clone, Copy)]
pub struct Size {
    pub value: Length,
    pub min: Length,
    pub max: Length,
}

#[derive(Debug, Clone, Copy)]
pub struct Border {
    pub width: Edges<f32>,
    pub color: Edges<Color>,
}

#[derive(Debug, Clone, Copy)]
pub struct FontId(pub u32);

#[derive(Debug, Clone, Copy)]
pub struct FontStyle {
    pub id: FontId,
    pub color: Color,
    pub size: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct Flex {
    pub direction: FlexDirection,
    pub grow: f32,
    pub shrink: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct Style {
    pub display: Option<Display>,
    pub position: Option<Position>,
    pub visibility: Option<Visibility>,
    pub width: Option<Length>,
    pub height: Option<Length>,
    pub min_width: Option<Length>,
    pub min_height: Option<Length>,
    pub max_width: Option<Length>,
    pub max_height: Option<Length>,

    pub background: Option<Color>,
    pub border: Option<Border>,
    pub font: Option<FontStyle>,
    pub flex_direction: Option<FlexDirection>,
    pub flex_grow: Option<f32>,
    pub flex_shrink: Option<f32>,

    pub margin: Option<Edges<f32>>,
    pub padding: Option<Edges<f32>>,

    pub overflow_x: Option<Overflow>,
    pub overlfow_y: Option<Overflow>,

    pub opacity: Option<f32>,
}

#[derive(Debug, Clone, Copy)]
pub struct ComputedStyle {
    pub display: Display,
    pub position: Position,
    pub visibility: Visibility,
    pub width: Size,
    pub height: Size,

    pub background: Color,
    pub border: Border,
    pub font: FontStyle,
    pub flex: Flex,

    pub margin: Edges<f32>,
    pub padding: Edges<f32>,

    pub overflow_x: Overflow,
    pub overlfow_y: Overflow,

    pub opacity: f32,
}

impl ComputedStyle {
    pub fn update(&mut self, style: &Style) {
        if let Some(v) = style.display {
            self.display = v
        };

        if let Some(v) = style.position {
            self.position = v
        };

        if let Some(v) = style.visibility {
            self.visibility = v
        };

        if let Some(v) = style.width {
            self.width.value = v
        };

        if let Some(v) = style.min_width {
            self.width.min = v
        };

        if let Some(v) = style.max_width {
            self.width.max = v
        };

        if let Some(v) = style.height {
            self.height.value = v
        };

        if let Some(v) = style.min_height {
            self.height.min = v
        };

        if let Some(v) = style.max_height {
            self.height.max = v
        };

        if let Some(v) = style.background {
            self.background = v
        };

        if let Some(v) = style.font {
            self.font = v
        };

        if let Some(v) = style.flex_direction {
            self.flex.direction = v
        };

        if let Some(v) = style.flex_grow {
            self.flex.grow = v
        };

        if let Some(v) = style.flex_shrink {
            self.flex.shrink = v
        };

        if let Some(v) = style.margin {
            self.margin = v
        };

        if let Some(v) = style.padding {
            self.padding = v
        };

        if let Some(v) = style.overflow_x {
            self.overflow_x = v
        };

        if let Some(v) = style.overlfow_y {
            self.overlfow_y = v
        };

        if let Some(v) = style.opacity {
            self.opacity = v
        };

        if let Some(v) = style.border {
            self.border = v
        };
    }
}

pub type Classname = &'static str;
pub type Class = Style;

pub struct ElementStyle {
    pub inline: Style,
    pub classes: SmallVec<[Class; 4]>,
}

pub struct ThemeColors;
pub struct ThemeSpacing;
pub struct ThemeRadii;
pub struct ThemeTypography;
pub struct Theme {
    pub colors: ThemeColors,
    pub spacing: ThemeSpacing,
    pub radii: ThemeRadii,
    pub typography: ThemeTypography,
    pub classes: HashMap<Classname, Class>,
}
