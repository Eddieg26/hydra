use crate::{
    core::ColorFormat,
    renderer::graph::RenderGraphMask,
    resources::{MainRenderTarget, RenderAssets, RenderTarget, RenderTexture},
    types::{Color, Viewport},
};
use asset::AssetId;
use ecs::{Component, Entity, Query, Resource, system::Removed};

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
    /// Render Graph Pass mask
    pub mask: Option<RenderGraphMask>,
    /// Generation, used to track changes in format or msaa
    /// settings of the render target.
    pub generation: u32,
}

impl CameraSettings {
    pub fn update(&mut self, camera: &Camera, target: &RenderTarget) {
        self.priority = camera.priority;
        self.target = camera.target;
        self.width = target.width;
        self.height = target.height;

        if self.format != target.format || self.msaa != camera.msaa || camera.target == None {
            self.format = target.format;
            self.msaa = camera.msaa;
            self.generation += 1;
        }
    }
}

impl Eq for CameraSettings {}
impl PartialEq for CameraSettings {
    fn eq(&self, other: &Self) -> bool {
        self.entity == other.entity
            && self.priority == other.priority
            && self.target == other.target
            && self.width == other.width
            && self.height == other.height
            && self.mask == other.mask
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
        cameras: Query<(Entity, &Camera, Option<&RenderGraphMask>)>,
        removed: Query<Entity, Removed<Camera>>,
        main_target: &MainRenderTarget,
        targets: &RenderAssets<RenderTarget>,
        queue: &mut CameraQueue,
    ) {
        queue.0.retain(|c| !removed.contains(c.entity));

        for (entity, camera, mask) in cameras {
            let target = match &camera.target {
                Some(id) => targets.get(id),
                None => main_target.get(),
            };

            let Some(target) = target else {
                continue;
            };

            if let Some(prev) = queue.0.iter_mut().find(|s| s.entity == entity) {
                prev.update(camera, target);
            } else {
                queue.0.push(CameraSettings {
                    entity,
                    priority: camera.priority,
                    target: camera.target,
                    msaa: camera.msaa,
                    width: target.width,
                    height: target.height,
                    format: target.format,
                    mask: mask.cloned(),
                    generation: 1,
                });
            };
        }

        queue.sort();
    }
}
