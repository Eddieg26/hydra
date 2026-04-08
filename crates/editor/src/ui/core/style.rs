use asset::AssetId;
use math::Vec2;
use renderer::types::Color;
use smallvec::SmallVec;
use std::collections::HashMap;

use crate::ui::core::Font;

#[derive(Debug, Clone, Copy)]
pub struct Edges<T> {
    pub left: T,
    pub right: T,
    pub top: T,
    pub bottom: T,
}

impl Edges<f32> {
    pub fn horizontal(&self) -> f32 {
        self.left + self.right
    }

    pub fn vertical(&self) -> f32 {
        self.top + self.bottom
    }

    pub fn scale(&self, size: Vec2) -> Edges<f32> {
        Self {
            left: self.left * size.x,
            right: self.right * size.x,
            top: self.top * size.y,
            bottom: self.bottom * size.y,
        }
    }
}

impl Edges<Length> {
    pub fn resolve(&self, size: Vec2, max: Option<Vec2>) -> Edges<f32> {
        Edges {
            left: self.left.resolve(size.x, max.map(|m| m.x)),
            right: self.right.resolve(size.x, max.map(|m| m.x)),
            top: self.top.resolve(size.y, max.map(|m| m.y)),
            bottom: self.bottom.resolve(size.y, max.map(|m| m.y)),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Length {
    Auto,
    Fixed(f32),
    Fill(f32),
}

impl Length {
    pub fn resolve(&self, auto: f32, max: Option<f32>) -> f32 {
        match self {
            Length::Auto => auto.min(max.unwrap_or(auto)),
            Length::Fixed(v) => *v,
            Length::Fill(v) => max.unwrap_or(auto) * *v,
        }
    }
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
pub struct Constrained<T = Length> {
    pub value: T,
    pub min: T,
    pub max: T,
}

impl<T> Constrained<T> {
    pub fn with(self, value: T) -> Self {
        Self {
            value,
            min: self.min,
            max: self.max,
        }
    }
}

impl Constrained<f32> {
    pub fn clamp(&self) -> f32 {
        self.value.clamp(self.min, self.max)
    }

    pub fn clamped(&self) -> (f32, bool) {
        let clamped = self.value < self.min || self.value > self.max;
        let value = self.value.clamp(self.min, self.max);
        (value, clamped)
    }
}

impl Constrained<Length> {
    pub fn resolve(&self, auto: f32, max: Option<f32>) -> Constrained<f32> {
        Constrained {
            value: self.value.resolve(auto, max),
            min: self.min.resolve(auto, max),
            max: self.max.resolve(auto, max),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Border {
    pub width: f32,
    pub color: Edges<Color>,
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextAlign {
    #[default]
    Left,
    Center,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontWeight {
    Light,
    Regular,
    Medium,
    Bold,
}

#[derive(Debug, Clone, Copy)]
pub struct TextStyle {
    pub font: Option<AssetId<Font>>,
    pub font_size: f32,
    pub line_height: f32,
    pub color: Color,
    pub align: TextAlign,
    pub weight: FontWeight,
    pub wrap_width: Option<f32>,
}

#[derive(Debug, Clone, Copy)]
pub struct Flex {
    pub direction: FlexDirection,
    pub grow: f32,
    pub shrink: f32,
}

impl From<FlexDirection> for Flex {
    fn from(direction: FlexDirection) -> Self {
        Self {
            direction,
            grow: 0.0,
            shrink: 0.0,
        }
    }
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlexWrap {
    #[default]
    None,
    Wrap,
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub enum Justify {
    #[default]
    Start,
    Center,
    End,
    Between,
    Around,
    Evenly,
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub enum Align {
    #[default]
    Start,
    Center,
    End,
    Stretch,
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
    pub text: Option<TextStyle>,
    pub flex_direction: Option<FlexDirection>,
    pub flex_grow: Option<f32>,
    pub flex_shrink: Option<f32>,
    pub justify: Option<Justify>,
    pub wrap: Option<FlexWrap>,
    pub align: Option<Align>,
    pub gap_x: Option<Length>,
    pub gap_y: Option<Length>,

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
    pub width: Constrained,
    pub height: Constrained,

    pub background: Color,
    pub border: Option<Border>,
    pub text: TextStyle,
    pub flex: Flex,
    pub wrap: FlexWrap,
    pub justify: Justify,
    pub align: Align,

    pub margin: Edges<f32>,
    pub padding: Edges<f32>,

    pub overflow_x: Overflow,
    pub overlfow_y: Overflow,
    pub gap_x: Length,
    pub gap_y: Length,

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

        if let Some(v) = style.text {
            self.text = v
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

        if let Some(v) = style.gap_x {
            self.gap_x = v
        };

        if let Some(v) = style.gap_y {
            self.gap_y = v
        };

        if let Some(v) = style.justify {
            self.justify = v
        }

        if let Some(v) = style.align {
            self.align = v;
        }

        if let Some(v) = style.wrap {
            self.wrap = v;
        }

        if let Some(v) = style.opacity {
            self.opacity = v
        };

        self.border = style.border;
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
