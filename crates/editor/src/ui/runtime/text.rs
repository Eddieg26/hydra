use crate::ui::core::{
    ElementId, Font, Glyph, GlyphKey, TextAlign,
    font::{GlyphQueue, GpuFont},
    style::{ComputedStyle, TextStyle},
};
use asset::AssetId;
use math::{Size, Vec2, rect::Rect};
use renderer::resources::RenderAssets;
use std::{collections::HashMap, ops::Range};

pub trait TextResolver {
    fn measure(&mut self, _id: ElementId, _text: &str, _style: &ComputedStyle) -> Size {
        Size::ZERO
    }
}

pub struct TextGlyph {
    pub rect: Rect,
    pub uv: Rect,
}

#[derive(Clone)]
pub struct TextLine {
    pub offset: f32,
    pub height: f32,
    pub range: Range<usize>,
}

impl TextLine {
    pub fn new(offset: f32, height: f32) -> Self {
        Self {
            offset,
            height,
            range: 0..0,
        }
    }
}

#[derive(Default)]
pub struct TextLayout {
    pub size: Size,
    pub lines: Vec<TextLine>,
    pub glyphs: Vec<TextGlyph>,
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct TextLayoutKey {
    pub font: Option<AssetId<Font>>,
    pub font_size: u32,
    pub line_height: u32,
    pub text: Box<str>,
}

pub struct TextLayoutCache(HashMap<TextLayoutKey, TextLayout>);

impl TextLayoutCache {
    pub fn get(&self, key: &TextLayoutKey) -> Option<&TextLayout> {
        self.0.get(key)
    }

    pub fn add(&mut self, key: TextLayoutKey, layout: TextLayout) {
        self.0.insert(key, layout);
    }
}

pub struct UITextSystem<'a> {
    map: HashMap<ElementId, TextLayoutKey>,
    cache: &'a mut TextLayoutCache,
    fonts: &'a mut RenderAssets<GpuFont>,
    queue: &'a mut GlyphQueue,
}

impl<'a> UITextSystem<'a> {
    pub fn layout_glyphs(&self, id: &ElementId, rect: &Rect, style: &TextStyle) -> Vec<TextGlyph> {
        let Some(layout) = self.map.get(id).and_then(|key| self.cache.get(key)) else {
            return vec![];
        };

        let offset = match style.align {
            TextAlign::Left => 0.0,
            TextAlign::Center => ((rect.width - layout.size.width) * 0.5).max(0.0),
            TextAlign::Right => (rect.width - layout.size.width).max(0.0),
        };

        layout
            .glyphs
            .iter()
            .map(|g| TextGlyph {
                rect: Rect::new(g.rect.x + offset, g.rect.y, g.rect.width, g.rect.height),
                uv: g.uv,
            })
            .collect()
    }

    pub fn compute_layout(&mut self, text: &str, style: &ComputedStyle) -> TextLayout {
        let UITextSystem { fonts, queue, .. } = self;

        let Some(id) = style.text.font else {
            return TextLayout::default();
        };

        let Some(font) = fonts.get_mut(&id) else {
            return TextLayout::default();
        };

        let line_height = match font.metrics(style.text.font_size) {
            Some(metrics) => metrics.new_line_size * style.text.line_height,
            None => style.text.font_size * style.text.line_height,
        };

        let mut state = TextLayoutState::new(&style.text);
        let mut glyphs = Vec::new();

        for (kind, token) in TokenCursor::new(text) {
            if kind == TokenKind::Newline {
                state.new_line(line_height);
                continue;
            }

            let missing = token.chars().filter_map(|ch| {
                let kern = state
                    .prev_char
                    .and_then(|p| font.kern(p, ch, state.font_size))
                    .unwrap_or(0.0);

                let (glyph, missing) = match font.glyph(ch, state.font_size).copied() {
                    Some(glyph) => (glyph, false),
                    None => (Glyph::default(), true),
                };

                let x = state.advance(line_height, state.cursor.x + kern + glyph.advance.x);
                let y = state.cursor.y - glyph.rect.y - glyph.rect.height;

                glyphs.push(TextGlyph {
                    rect: Rect::new(x, y, glyph.rect.width, glyph.rect.height),
                    uv: glyph.uv,
                });

                state.prev_char = Some(ch);
                missing.then_some(GlyphKey::new(ch, state.font_size))
            });

            queue.extend(id, missing);
        }

        let width = state.line_width;
        let height = state.lines.len() as f32 * line_height;

        TextLayout {
            size: Size::new(width, height),
            lines: state.lines,
            glyphs,
        }
    }
}

impl TextResolver for UITextSystem<'_> {
    fn measure(&mut self, id: ElementId, text: &str, style: &ComputedStyle) -> Size {
        let key = TextLayoutKey {
            font: style.text.font,
            font_size: style.text.font_size.to_bits(),
            line_height: style.text.line_height.to_bits(),
            text: Box::from(text),
        };

        self.map.insert(id, key.clone());

        match self.cache.get(&key) {
            Some(layout) => layout.size,
            None => {
                let layout = self.compute_layout(text, style);
                let size = layout.size;
                self.cache.add(key, layout);
                size
            }
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

pub struct TextLayoutState {
    cursor: Vec2,
    lines: Vec<TextLine>,
    line_width: f32,
    font_size: u32,
    max_width: f32,
    current_line: TextLine,
    prev_char: Option<char>,
    wrap: bool,
}

impl TextLayoutState {
    pub fn new(style: &TextStyle) -> Self {
        Self {
            cursor: Vec2::ZERO,
            lines: Vec::new(),
            line_width: 0.0,
            font_size: style.font_size.to_bits(),
            max_width: match style.wrap_width {
                Some(width) => width.max(0.0),
                None => f32::INFINITY,
            },
            current_line: TextLine::new(0.0, style.line_height),
            prev_char: None,
            wrap: style.wrap_width.is_some(),
        }
    }

    pub fn new_line(&mut self, line_height: f32) -> Vec2 {
        let y = self.cursor.y + line_height;
        let new_line = TextLine::new(y, line_height);
        let old_line = std::mem::replace(&mut self.current_line, new_line);

        self.lines.push(old_line);
        self.line_width = self.line_width.max(self.cursor.x);
        self.cursor.x = 0.0;
        self.cursor.y = y;
        self.cursor
    }

    pub fn advance(&mut self, line_height: f32, x: f32) -> f32 {
        if x > self.max_width && self.wrap {
            let y = self.cursor.y + line_height;
            let new_line = TextLine::new(y, line_height);
            let old_line = std::mem::replace(&mut self.current_line, new_line);

            self.lines.push(old_line);
            self.cursor.x = 0.0;
            self.cursor.y = y;
            self.cursor.x
        } else {
            let pos = self.cursor.x;
            self.cursor.x = x;
            pos
        }
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
}
