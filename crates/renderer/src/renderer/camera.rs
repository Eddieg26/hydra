use crate::{
    core::ColorFormat,
    resources::{MainRenderTarget, RenderAssets, RenderTarget, RenderTexture},
    types::{Color, Viewport},
};
use asset::AssetId;
use ecs::{
    Component, Entity, Query, Resource,
    system::{Added, Modified, Removed},
};

#[derive(Debug, Clone, Copy)]
pub enum Projection {
    Perspective { fov: f32, near: f32, far: f32 },
    Orthographic { size: f32, near: f32, far: f32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct CameraSettings {
    /// Camera Entity
    pub entity: Entity,
    /// Render priority, higher priority cameras are rendered first
    pub priority: i8,
    /// Render target, if None - render to screen
    pub target: Option<AssetId<RenderTexture>>,
    /// MSAA samples, if Auto - use the same value as the main render target
    pub msaa: SettingState,
    /// HDR rendering, if Auto - use the same value as the main render target
    pub hdr: SettingState,
    /// Render target width
    pub width: u32,
    /// Render target width
    pub height: u32,
    /// Render target color format
    pub format: ColorFormat,
}

#[derive(Resource, Default, Clone, PartialEq, Eq, Hash)]
pub struct CameraQueue(Vec<CameraSettings>);
impl CameraQueue {
    pub fn sort(&mut self) {
        self.0.sort_by(|a, b| a.priority.cmp(&b.priority).reverse());
    }

    pub fn slice(&self) -> &[CameraSettings] {
        &self.0
    }

    pub fn owned(self) -> Vec<CameraSettings> {
        self.0
    }
}

impl From<CameraQueue> for Vec<CameraSettings> {
    fn from(value: CameraQueue) -> Self {
        value.0
    }
}

impl CameraQueue {
    pub(crate) fn queue(
        added: Query<(Entity, &Camera), Added<Camera>>,
        modified: Query<(Entity, &Camera), Modified<Camera>>,
        removed: Query<Entity, Removed<Camera>>,
        main_target: &MainRenderTarget,
        targets: &RenderAssets<RenderTarget>,
        queue: &mut CameraQueue,
    ) {
        let removed = removed.iter().collect::<Vec<_>>();
        queue.0.retain(|c| !removed.contains(&c.entity));

        for (entity, camera) in modified {
            let Some(prev) = queue.0.iter_mut().find(|s| s.entity == entity) else {
                continue;
            };

            let target = match &camera.target {
                Some(id) => targets.get(id),
                None => main_target.get(),
            };

            let Some(target) = target else {
                continue;
            };

            if camera.target != prev.target
                || target.width != prev.width
                || target.height != prev.height
                || target.format != prev.format
            {
                *prev = CameraSettings {
                    entity,
                    priority: camera.priority,
                    target: camera.target,
                    msaa: camera.msaa,
                    hdr: camera.hdr,
                    width: target.width,
                    height: target.height,
                    format: target.format,
                }
            }
        }

        for (entity, camera) in added {
            let target = match &camera.target {
                Some(id) => targets.get(id),
                None => main_target.get(),
            };

            let Some(target) = target else {
                continue;
            };

            queue.0.push(CameraSettings {
                entity,
                priority: camera.priority,
                target: camera.target,
                msaa: camera.msaa,
                hdr: camera.hdr,
                width: target.width,
                height: target.height,
                format: target.format,
            });
        }

        queue.sort();
    }
}
