use std::ops::Range;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Viewport {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub depth: Range<f32>,
}
