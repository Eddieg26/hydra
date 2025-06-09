use crate::{
    renderer::graph::SubGraph,
    resources::RenderTexture,
    types::{Color, Viewport},
};
use asset::AssetId;
use ecs::{Component, Entity, Resource, app::Main, system::unlifetime::SQuery};

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Projection {
    Orthographic {
        left: f32,
        right: f32,
        bottom: f32,
        top: f32,
        near: f32,
        far: f32,
    },
    Perspective {
        fov: f32, // in radians
        aspect_ratio: f32,
        near: f32,
        far: f32,
    },
}

#[derive(Debug, Clone, Component, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Camera {
    pub viewport: Viewport,
    pub depth: u32,
    pub clear_color: Option<Color>,
    pub target: Option<AssetId<RenderTexture>>,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            viewport: Viewport::new(0.0, 0.0, 1.0, 1.0, 0.0..1.0),
            depth: Default::default(),
            clear_color: Default::default(),
            target: Default::default(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct EntityCamera {
    pub entity: Entity,
    camera: Camera,
}

impl std::ops::Deref for EntityCamera {
    type Target = Camera;

    fn deref(&self) -> &Self::Target {
        &self.camera
    }
}

impl std::ops::DerefMut for EntityCamera {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.camera
    }
}

#[derive(Default, Resource)]
pub struct EntityCameras {
    cameras: Vec<EntityCamera>,
}

impl EntityCameras {
    pub(crate) fn extract(
        cameras: Main<SQuery<(Entity, &Camera)>>,
        entity_cameras: &mut EntityCameras,
    ) {
        entity_cameras.cameras.clear();

        for (entity, camera) in cameras.iter() {
            entity_cameras.cameras.push(EntityCamera {
                entity,
                camera: camera.clone(),
            });
        }

        entity_cameras.cameras.sort_by_key(|c| c.camera.depth);
    }
}

impl<'a> IntoIterator for &'a EntityCameras {
    type Item = &'a EntityCamera;
    type IntoIter = std::slice::Iter<'a, EntityCamera>;

    fn into_iter(self) -> Self::IntoIter {
        self.cameras.iter()
    }
}

pub struct CameraSubGraph;

impl SubGraph for CameraSubGraph {
    const NAME: super::graph::Name = "CameraSubGraph";

    fn run(ctx: &mut super::graph::RenderContext) {
        for camera in ctx.world().resource::<EntityCameras>().into_iter().cloned() {
            ctx.set_camera(camera);
            ctx.run_sub_graph(Self::NAME);
        }
    }
}
