use crate::{
    resources::RenderTexture,
    types::{Color, Viewport},
};
use asset::AssetId;
use ecs::Component;

#[derive(Debug, Clone, Copy)]
pub enum Projection {
    Perspective { fov: f32, near: f32, far: f32 },
    Orthographic { size: f32, near: f32, far: f32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingState {
    Auto,
    Disabled,
}

#[derive(Debug, Component)]
pub struct Camera {
    pub projection: Projection,

    pub viewport: Viewport,

    pub priority: i8,

    pub target: Option<AssetId<RenderTexture>>,

    pub clear: Option<Color>,

    pub msaa: SettingState,

    pub hdr: SettingState,
}
