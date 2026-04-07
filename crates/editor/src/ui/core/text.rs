use crate::ui::core::{
    GlyphKey,
    font::{GlyphQueue, GpuFont},
    style::{ComputedStyle, TextStyle},
};
use math::{Size, rect::Rect};
use renderer::resources::RenderAssets;

#[derive(Default)]
pub struct TextMeasurement {
    pub size: Size,
    pub line_count: u32,
}

pub trait TextMeasurer {
    fn measure(&mut self, _text: &str, _style: &ComputedStyle) -> TextMeasurement {
        TextMeasurement::default()
    }
}

pub struct TextGlyph {
    pub rect: Rect,
    pub uv: Rect,
}

#[derive(Default)]
pub struct TextLayout {
    pub size: Size,
    pub lines: u32,
    pub glyphs: Vec<TextGlyph>,
}

pub struct TextSystem<'a> {
    fonts: &'a mut RenderAssets<GpuFont>,
    queue: &'a mut GlyphQueue,
}

impl<'a> TextMeasurer for TextSystem<'a> {
    fn measure(&mut self, text: &str, style: &ComputedStyle) -> TextMeasurement {
        let TextSystem { fonts, queue } = self;

        let Some(id) = style.text.font else {
            return TextMeasurement::default();
        };

        let Some(font) = fonts.get_mut(&id) else {
            return TextMeasurement::default();
        };

        let mut state = TextMeasureState::new(&style.text);

        for (kind, token) in TokenCursor::new(text) {
            if kind == TokenKind::Newline {
                state.new_line();
                continue;
            }

            if style.text.wrap_width.is_some() {
                let token_width = state.measure_token(font, token);
                if state.should_wrap(token, token_width) {
                    state.new_line();
                }
            }

            for entry in state.advance(font, token) {
                queue.queue(id, entry);
            }
        }

        state.measurement()
    }
}

pub struct TextMeasureState {
    pen_x: f32,
    line_width: f32,
    line_count: u32,
    prev: Option<char>,
    font_size: u32,
    line_height: f32,
    max_width: f32,
}

impl TextMeasureState {
    pub fn new(style: &TextStyle) -> Self {
        Self {
            pen_x: 0.0,
            line_width: 0.0,
            line_count: 1,
            prev: None,
            font_size: style.font_size.to_bits(),
            line_height: style.line_height,
            max_width: match style.wrap_width {
                Some(width) => width.max(0.0),
                None => f32::INFINITY,
            },
        }
    }

    pub fn advance(&mut self, font: &mut GpuFont, token: &str) -> impl Iterator<Item = GlyphKey> {
        let size = self.font_size;

        token.chars().filter_map(move |ch| {
            if let Some(prev_ch) = self.prev {
                self.pen_x += font.kern(prev_ch, ch, size).unwrap_or(0.0);
            }

            let (advance, missing) = match font.glyph(ch, size).map(|g| g.advance.x) {
                Some(advance) => (advance, false),
                None => (0.0, true),
            };

            self.pen_x += advance;
            self.prev = Some(ch);

            missing.then_some(GlyphKey { ch, size })
        })
    }

    pub fn should_wrap(&self, token: &str, token_width: f32) -> bool {
        self.pen_x > 0.0 && self.pen_x + token_width > self.max_width && !token.trim().is_empty()
    }

    pub fn new_line(&mut self) {
        self.line_width = self.line_width.max(self.pen_x);
        self.pen_x = 0.0;
        self.prev = None;
        self.line_count += 1;
    }

    pub fn measure_token(&self, font: &mut GpuFont, token: &str) -> f32 {
        let mut width = 0.0f32;
        let mut prev = None::<char>;

        for ch in token.chars() {
            if let Some(prev_ch) = prev {
                width += font.kern(prev_ch, ch, self.font_size).unwrap_or(0.0);
            }

            width += font
                .glyph(ch, self.font_size)
                .map(|g| g.advance.x)
                .unwrap_or(0.0);
            prev = Some(ch);
        }

        width
    }

    pub fn measurement(self) -> TextMeasurement {
        let width = self.line_width.max(self.pen_x);
        let height = self.line_height * self.line_count as f32;

        TextMeasurement {
            size: Size::new(width, height),
            line_count: self.line_count,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TokenKind {
    Word,
    Whitespace,
    Newline,
}

/// Zero-allocation iterator that yields `&str` tokens (words, whitespace runs, newlines)
/// by slicing directly into the source text.
pub struct TokenCursor<'a> {
    text: &'a str,
    pos: usize,
}

impl<'a> TokenCursor<'a> {
    pub fn new(text: &'a str) -> Self {
        Self { text, pos: 0 }
    }
}

impl<'a> Iterator for TokenCursor<'a> {
    type Item = (TokenKind, &'a str);

    fn next(&mut self) -> Option<Self::Item> {
        let remaining = &self.text[self.pos..];
        let mut chars = remaining.char_indices();
        let (_, first) = chars.next()?;

        if first == '\n' {
            self.pos += 1;
            return Some((TokenKind::Newline, &self.text[self.pos - 1..self.pos]));
        }

        let start = self.pos;
        let is_ws = first.is_whitespace();

        for (i, ch) in chars {
            if ch == '\n' || ch.is_whitespace() != is_ws {
                self.pos = start + i;
                return Some((
                    if is_ws {
                        TokenKind::Whitespace
                    } else {
                        TokenKind::Word
                    },
                    &self.text[start..self.pos],
                ));
            }
        }

        self.pos = self.text.len();
        Some((
            if is_ws {
                TokenKind::Whitespace
            } else {
                TokenKind::Word
            },
            &self.text[start..self.pos],
        ))
    }
}
