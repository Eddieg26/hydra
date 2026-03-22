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
}

#[derive(Clone, Hash)]
pub struct CameraSettings {
    /// Camera Entity
    pub entity: Entity,
    /// Render priority, higher priority cameras are rendered first
    pub priority: i8,
    /// Render target, if None - render to screen
    pub target: Option<AssetId<RenderTexture>>,
    /// MSAA samples, if Auto - use the same value as the main render target
    pub msaa: SettingState,
    /// Render target width
    pub width: u32,
    /// Render target width
    pub height: u32,
    /// Render target color format
    pub format: ColorFormat,
    pub generation: u32,
}

impl Eq for CameraSettings {}
impl PartialEq for CameraSettings {
    fn eq(&self, other: &Self) -> bool {
        self.entity == other.entity
            && self.priority == other.priority
            && self.target == other.target
            && self.width == other.width
            && self.height == other.height
            && self.generation == other.generation
    }
}

impl CameraSettings {
    pub fn diff(&self, format: ColorFormat, msaa: SettingState) -> bool {
        self.format != format || self.msaa != msaa
    }
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

        let mut new = Vec::new();
        for (entity, camera) in modified {
            let Some(prev) = queue.0.iter_mut().find(|s| s.entity == entity) else {
                new.push((entity, camera));
                continue;
            };

            let target = match &camera.target {
                Some(id) => targets.get(id),
                None => main_target.get(),
            };

            let Some(target) = target else {
                continue;
            };

            prev.priority = camera.priority;
            prev.target = camera.target;
            prev.width = target.width;
            prev.height = target.height;
            if prev.format != target.format || prev.msaa != camera.msaa {
                prev.format = target.format;
                prev.msaa = camera.msaa;
                prev.generation += 1;
            }
        }

        for (entity, camera) in added.iter().chain(new) {
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
                width: target.width,
                height: target.height,
                format: target.format,
                generation: 1,
            });
        }

        queue.sort();
    }
}
