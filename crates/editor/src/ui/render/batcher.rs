use crate::ui::{
    core::{style::Border, font::Glyph},
    render::pipelines::UiPipelines,
    runtime::paint::DrawCommand,
};
use asset::AssetId;
use math::{Vec2, rect::Rect};
use renderer::{
    resources::{PipelineId, Texture},
    types::Color,
};
use std::ops::Range;

pub struct UIVertex {
    pub position: Vec2,
    pub uv: Vec2,
    pub color: u32,
}

impl UIVertex {
    pub fn new(position: Vec2, uv: Vec2, color: u32) -> Self {
        Self {
            position,
            uv,
            color,
        }
    }

    pub fn quad(rect: Rect, uv: Rect, color: u32) -> [Self; 4] {
        [
            Self::new(Vec2::new(rect.x, rect.y), Vec2::new(uv.x, uv.y), color),
            Self::new(
                Vec2::new(rect.x + rect.width, rect.y),
                Vec2::new(uv.x + uv.width, uv.y),
                color,
            ),
            Self::new(
                Vec2::new(rect.x + rect.width, rect.y + rect.height),
                Vec2::new(uv.x + uv.width, uv.y + uv.height),
                color,
            ),
            Self::new(
                Vec2::new(rect.x, rect.y + rect.height),
                Vec2::new(uv.x, uv.y + uv.height),
                color,
            ),
        ]
    }
}

pub struct UIDrawBatch {
    pub pipeline: PipelineId,
    pub vertices: Range<u32>,
    pub indices: Range<u32>,
    pub clip: Rect<u32>,
    pub image: Option<AssetId<Texture>>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum UIBatchType {
    Solid,
    Image,
    Text,
}

#[derive(Clone, PartialEq, Eq)]
pub struct UIBatchInfo {
    ty: UIBatchType,
    image: Option<AssetId<Texture>>,
    vertices: Range<u32>,
    indices: Range<u32>,
    clip: Rect<u32>,
}

impl UIBatchInfo {
    pub fn new(
        ty: UIBatchType,
        clip: Rect<u32>,
        start_vertex: u32,
        start_index: u32,
        image: Option<AssetId<Texture>>,
    ) -> Self {
        Self {
            ty,
            image,
            vertices: start_vertex..start_vertex,
            indices: start_index..start_index,
            clip: clip,
        }
    }

    pub fn solid(clip: Rect<u32>, start_vertex: u32, start_index: u32) -> Self {
        Self {
            ty: UIBatchType::Solid,
            image: None,
            vertices: start_vertex..start_vertex,
            indices: start_index..start_index,
            clip: clip,
        }
    }

    pub fn image(
        image: AssetId<Texture>,
        clip: Rect<u32>,
        start_vertex: u32,
        start_index: u32,
    ) -> Self {
        Self {
            ty: UIBatchType::Image,
            image: Some(image),
            vertices: start_vertex..start_vertex,
            indices: start_index..start_index,
            clip,
        }
    }

    pub fn text(
        image: AssetId<Texture>,
        clip: Rect<u32>,
        start_vertex: u32,
        start_index: u32,
    ) -> Self {
        Self {
            ty: UIBatchType::Text,
            image: Some(image),
            vertices: start_vertex..start_vertex,
            indices: start_index..start_index,
            clip: clip,
        }
    }

    pub fn equal(
        &self,
        ty: UIBatchType,
        clip: &Rect<u32>,
        image: Option<&AssetId<Texture>>,
    ) -> bool {
        self.ty == ty && self.image.as_ref() == image && &self.clip == clip
    }
}
pub struct UIBatchOutput {
    pub vertices: Vec<UIVertex>,
    pub indices: Vec<u32>,
    pub batches: Vec<UIDrawBatch>,
}

pub struct Batcher<'a> {
    pipelines: &'a UiPipelines,
    commands: &'a [DrawCommand],
    vertices: Vec<UIVertex>,
    indices: Vec<u32>,
    batches: Vec<UIDrawBatch>,
}

impl Batcher<'_> {
    pub fn run(mut self) -> UIBatchOutput {
        let mut current: Option<UIBatchInfo> = None;
        let mut clips = Vec::new();

        for command in self.commands {
            match command {
                DrawCommand::PushClip(rect) => {
                    current = self.submit(current);
                    clips.push(*rect);
                }
                DrawCommand::PopClip => {
                    current = self.submit(current);
                    clips.pop();
                }
                DrawCommand::Quad { rect, color } => {
                    let clip = Self::clip(&mut clips, rect).round();
                    let info = match current
                        .as_mut()
                        .and_then(|c| c.equal(UIBatchType::Solid, &clip, None).then_some(c))
                    {
                        Some(current) => current,
                        None => {
                            let info = UIBatchInfo::solid(
                                clip,
                                self.vertices.len() as u32,
                                self.vertices.len() as u32,
                            );

                            current = self.submit(current).or(Some(info));
                            current.as_mut().unwrap()
                        }
                    };

                    self.add_quad(UIVertex::quad(*rect, Rect::ONE, color.pack()), info);
                }
                DrawCommand::Image { id, rect } => {
                    let clip = Self::clip(&mut clips, rect).round();
                    let info = match current
                        .as_mut()
                        .and_then(|c| c.equal(UIBatchType::Image, &clip, Some(id)).then_some(c))
                    {
                        Some(current) => current,
                        None => {
                            let info = UIBatchInfo::image(
                                *id,
                                clip,
                                self.vertices.len() as u32,
                                self.indices.len() as u32,
                            );

                            current = self.submit(current).or(Some(info));
                            current.as_mut().unwrap()
                        }
                    };

                    self.add_quad(UIVertex::quad(*rect, Rect::ONE, Color::WHITE.pack()), info);
                }
                DrawCommand::Text {
                    rect,
                    atlas,
                    glyphs,
                } => {
                    let clip = Self::clip(&mut clips, rect).round();
                    let info = match current
                        .as_mut()
                        .and_then(|c| c.equal(UIBatchType::Text, &clip, Some(atlas)).then_some(c))
                    {
                        Some(current) => current,
                        None => {
                            let info = UIBatchInfo::text(
                                *atlas,
                                clip,
                                self.vertices.len() as u32,
                                self.indices.len() as u32,
                            );

                            current = self.submit(current).or(Some(info));
                            current.as_mut().unwrap()
                        }
                    };

                    self.add_text(rect, glyphs, info);
                }
                DrawCommand::Border { rect, border } => {
                    let clip = Self::clip(&mut clips, rect).round();
                    let info = match current
                        .as_mut()
                        .and_then(|c| c.equal(UIBatchType::Solid, &clip, None).then_some(c))
                    {
                        Some(current) => current,
                        None => {
                            let info = UIBatchInfo::solid(
                                clip,
                                self.vertices.len() as u32,
                                self.indices.len() as u32,
                            );

                            current = self.submit(current).or(Some(info));
                            current.as_mut().unwrap()
                        }
                    };

                    self.add_border(rect, border, info);
                }
            }
        }

        self.submit(current);

        UIBatchOutput {
            vertices: self.vertices,
            indices: self.indices,
            batches: self.batches,
        }
    }

    fn submit(&mut self, info: Option<UIBatchInfo>) -> Option<UIBatchInfo> {
        if let Some(info) = info {
            let pipeline = match info.ty {
                UIBatchType::Solid => self.pipelines.solid(),
                UIBatchType::Image => self.pipelines.image(),
                UIBatchType::Text => self.pipelines.text(),
            };

            self.batches.push(UIDrawBatch {
                pipeline,
                vertices: info.vertices,
                indices: info.indices,
                clip: info.clip,
                image: info.image,
            });
        }

        None
    }

    fn clip(clips: &mut Vec<Rect>, rect: &Rect) -> Rect {
        if let Some(parent) = clips.last().copied() {
            parent.intersection(&rect)
        } else {
            *rect
        }
    }

    fn add_quad(&mut self, quad: [UIVertex; 4], info: &mut UIBatchInfo) {
        let base_vertex = self.vertices.len() as u32;

        self.vertices.extend(quad);
        self.indices.extend_from_slice(&[
            base_vertex + 0,
            base_vertex + 1,
            base_vertex + 2,
            base_vertex + 0,
            base_vertex + 2,
            base_vertex + 3,
        ]);

        info.vertices.end = self.vertices.len() as u32;
        info.indices.end = self.indices.len() as u32;
    }

    fn add_border(&mut self, rect: &Rect, border: &Border, info: &mut UIBatchInfo) {
        let top = UIVertex::quad(
            Rect {
                x: rect.x,
                y: rect.y,
                width: rect.width,
                height: border.width,
            },
            Rect::ZERO,
            Color::WHITE.pack(),
        );

        let bottom = UIVertex::quad(
            Rect {
                x: rect.x,
                y: rect.y + rect.height,
                width: rect.width,
                height: border.width,
            },
            Rect::ZERO,
            Color::WHITE.pack(),
        );

        let left = UIVertex::quad(
            Rect {
                x: rect.x,
                y: rect.y,
                width: border.width,
                height: rect.height,
            },
            Rect::ZERO,
            Color::WHITE.pack(),
        );

        let right = UIVertex::quad(
            Rect {
                x: rect.x + rect.width,
                y: rect.y,
                width: border.width,
                height: rect.height,
            },
            Rect::ZERO,
            Color::WHITE.pack(),
        );

        self.add_quad(top, info);
        self.add_quad(bottom, info);
        self.add_quad(left, info);
        self.add_quad(right, info);
    }

    fn add_text(&mut self, rect: &Rect, glyphs: &[Glyph], info: &mut UIBatchInfo) {
        for glyph in glyphs {
            let rect = Rect::new(
                rect.x + glyph.rect.x,
                rect.y + glyph.rect.y,
                glyph.rect.x,
                glyph.rect.y,
            );

            self.add_quad(UIVertex::quad(rect, glyph.uv, Color::WHITE.pack()), info);
        }
    }
}
