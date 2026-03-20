use crate::{
    resources::RenderTexture,
    types::{Color, Viewport},
};
use asset::AssetId;
use ecs::Component;
use wgpu::TextureView;

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

#[derive(Debug, Clone, Component)]
pub struct Camera {
    /// Projection matrix type and parameters
    pub projection: Projection,
    /// Viewport in normalized coordinates (0.0..1.0)
    pub viewport: Viewport,
    /// Render priority, higher priority cameras are rendered first
    pub priority: i8,
    /// Render target, if None - render to screen
    pub target: Option<AssetId<RenderTexture>>,
    /// Clear color, if None - don't clear
    pub clear: Option<Color>,
    /// MSAA samples, if Auto - use the same value as the main render target
    pub msaa: SettingState,
    /// HDR rendering, if Auto - use the same value as the main render target
    pub hdr: SettingState,
}

#[derive(Debug, Clone, Component)]
pub struct RenderTargetAttachments {
    /// MSAA color texture, if MSAA is enabled
    pub color_msaa: Option<TextureView>,
    /// Color texture
    pub color: TextureView,
    /// Output texture, final result of the render pass
    pub output: TextureView,
}
