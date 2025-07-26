use serde::{Deserialize, Serialize};
use std::ops::Range;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Viewport {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub depth: Range<f32>,
}

impl Viewport {
    pub fn new(x: f32, y: f32, width: f32, height: f32, depth: Range<f32>) -> Self {
        Self {
            x,
            y,
            width,
            height,
            depth,
        }
    }
}
