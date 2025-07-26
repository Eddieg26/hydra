use crate::{
    RenderAssets, RenderDevice, RenderGraphError, RenderSurface, RenderSurfaceTexture,
    RenderTarget,
    primitives::{Color, Viewport},
    renderer::graph::SubGraph,
    resources::RenderTexture,
};
use asset::AssetId;
use ecs::{Component, Entity, IndexMap, Query, Resource, app::sync::MainEntity};
use math::{Mat4, Size, Vec3, Vec3A, Vec4, bounds::Bounds, sphere::Sphere};

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Projection {
    Orthographic {
        near: f32,
        far: f32,
        size: f32,
    },
    Perspective {
        fov: f32, // in radians
        near: f32,
        far: f32,
    },
}

impl Projection {
    pub fn perspective(fov: f32, near: f32, far: f32) -> Self {
        Projection::Perspective { fov, near, far }
    }

    pub fn orthographic(near: f32, far: f32, size: f32) -> Self {
        Projection::Orthographic { near, far, size }
    }

    pub fn near(&self) -> f32 {
        match self {
            Projection::Orthographic { near, .. } => *near,
            Projection::Perspective { near, .. } => *near,
        }
    }

    pub fn far(&self) -> f32 {
        match self {
            Projection::Orthographic { far, .. } => *far,
            Projection::Perspective { far, .. } => *far,
        }
    }

    pub fn frustum_points(&self, width: u32, height: u32, camera: &Camera) -> [Vec3A; 8] {
        match *self {
            Projection::Orthographic { near, far, .. } => {
                let left = width as f32 * camera.viewport.x;
                let top = height as f32 * camera.viewport.y;
                let right = left + width as f32 * camera.viewport.width;
                let bottom = top + height as f32 * camera.viewport.height;

                // NOTE: These vertices are in the specific order required by [`calculate_cascade`].
                [
                    Vec3A::new(right, bottom, near), // bottom right
                    Vec3A::new(right, top, near),    // top right
                    Vec3A::new(left, top, near),     // top left
                    Vec3A::new(left, bottom, near),  // bottom left
                    Vec3A::new(right, bottom, far),  // bottom right
                    Vec3A::new(right, top, far),     // top right
                    Vec3A::new(left, top, far),      // top left
                    Vec3A::new(left, bottom, far),   // bottom left
                ]
            }
            Projection::Perspective { fov, near, far } => {
                let aspect_ratio = width as f32 / height as f32;
                let tan_half_fov = f32::tan(fov / 2.0);
                let a = near.abs() * tan_half_fov;
                let b = far.abs() * tan_half_fov;
                // NOTE: These vertices are in the specific order required by [`calculate_cascade`].
                [
                    Vec3A::new(a * aspect_ratio, -a, near),  // bottom right
                    Vec3A::new(a * aspect_ratio, a, near),   // top right
                    Vec3A::new(-a * aspect_ratio, a, near),  // top left
                    Vec3A::new(-a * aspect_ratio, -a, near), // bottom left
                    Vec3A::new(b * aspect_ratio, -b, far),   // bottom right
                    Vec3A::new(b * aspect_ratio, b, far),    // top right
                    Vec3A::new(-b * aspect_ratio, b, far),   // top left
                    Vec3A::new(-b * aspect_ratio, -b, far),  // bottom left
                ]
            }
        }
    }
}

#[derive(Debug, Clone, Component, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Camera {
    pub viewport: Viewport,
    pub order: u32,
    pub clear_color: Option<Color>,
    pub target: Option<AssetId<RenderTexture>>,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            viewport: Viewport::new(0.0, 0.0, 1.0, 1.0, 0.0..1.0),
            order: Default::default(),
            clear_color: Default::default(),
            target: Default::default(),
        }
    }
}

pub struct CameraRenderTarget {
    pub size: Size<u32>,
    pub color: Option<wgpu::TextureView>,
    pub depth: wgpu::TextureView,
    pub clear: Option<Color>,
    pub target: Option<AssetId<RenderTexture>>,
    pub order: u32,
}

#[derive(Default, Resource)]
pub struct CameraRenderTargets {
    targets: IndexMap<Entity, CameraRenderTarget>,
}

impl CameraRenderTargets {
    pub fn get(&self, entity: &Entity) -> Option<&CameraRenderTarget> {
        self.targets.get(entity)
    }

    pub fn contains(&self, entity: &Entity) -> bool {
        self.targets.contains_key(entity)
    }

    pub(crate) fn queue(
        targets: &mut Self,
        cameras: Query<(&MainEntity, &Camera)>,
        device: &RenderDevice,
        surface: &RenderSurface,
        surface_texture: &RenderSurfaceTexture,
        render_targets: &RenderAssets<RenderTarget>,
    ) {
        targets
            .targets
            .retain(|t, _| cameras.into_iter().any(|(e, _)| &e.0 == t));

        for (entity, camera) in cameras.iter() {
            let remove = if let Some(id) = camera.target {
                Self::add_render_target(
                    targets,
                    device,
                    &entity.0,
                    camera,
                    surface,
                    id,
                    render_targets,
                )
            } else {
                Self::add_surface_target(
                    targets,
                    device,
                    &entity.0,
                    camera,
                    surface,
                    surface_texture,
                )
            };

            remove.then_some(|| targets.targets.shift_remove(&entity.0));
        }

        targets.targets.sort_by(|_, a, _, b| a.order.cmp(&b.order));
    }

    /// Remove old surface render targets to avoid accessing destroyed textures.
    /// This is necessary because the surface texture is recreated every frame.
    /// If the render target uses a surface texture, we remove the target from the list.
    pub(crate) fn cleanup(targets: &mut Self) {
        targets.targets.values_mut().for_each(|t| {
            if t.target.is_none() {
                t.color = None;
            }
        });
    }

    #[inline]
    fn add_surface_target(
        targets: &mut Self,
        device: &RenderDevice,
        entity: &Entity,
        camera: &Camera,
        surface: &RenderSurface,
        surface_texture: &RenderSurfaceTexture,
    ) -> bool {
        let Some(texture) = surface_texture.get() else {
            return true;
        };

        match targets.targets.get_mut(entity) {
            Some(target) if target.size == surface.size() && target.target.is_some() => {
                target.color = Some(texture.texture.create_view(&Default::default()))
            }
            _ => {
                let color = Some(texture.texture.create_view(&Default::default()));
                let depth =
                    Self::create_depth_texture(device, surface.size(), surface.depth_format());

                let target = CameraRenderTarget {
                    size: surface.size(),
                    color,
                    depth: depth.create_view(&Default::default()),
                    clear: camera.clear_color,
                    target: camera.target,
                    order: camera.order,
                };

                targets.targets.insert(*entity, target);
            }
        }

        false
    }

    #[inline]
    fn add_render_target(
        targets: &mut Self,
        device: &RenderDevice,
        entity: &Entity,
        camera: &Camera,
        surface: &RenderSurface,
        target: AssetId<RenderTexture>,
        render_targets: &RenderAssets<RenderTarget>,
    ) -> bool {
        let Some(target) = render_targets.get(&target.into()) else {
            return true;
        };

        if !targets
            .get(entity)
            .is_some_and(|t| t.target == camera.target)
        {
            let size = Size::new(target.width(), target.height());
            let color = Some(target.texture().create_view(&Default::default()));
            let depth = Self::create_depth_texture(device, size, surface.depth_format());

            let target = CameraRenderTarget {
                size,
                color,
                depth: depth.create_view(&Default::default()),
                clear: camera.clear_color,
                target: camera.target,
                order: camera.order,
            };

            targets.targets.insert(*entity, target);
        }

        false
    }

    fn create_depth_texture(
        device: &RenderDevice,
        size: Size<u32>,
        format: wgpu::TextureFormat,
    ) -> wgpu::Texture {
        device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width: size.width,
                height: size.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        })
    }
}

pub struct CameraSubGraph;

impl SubGraph for CameraSubGraph {
    const NAME: super::graph::Name = "CameraSubGraph";

    fn run(ctx: &mut super::graph::RenderContext) -> Result<(), RenderGraphError> {
        let cameras = ctx.world().resource::<CameraRenderTargets>();
        for entity in cameras.targets.keys() {
            ctx.set_view(*entity);
            let _ = ctx.run_sub_graph(Self::NAME);
        }

        Ok(())
    }
}

/// A plane is represented as a 4D vector: ax + by + cz + d = 0
/// Where (a, b, c) is the normal and d is the distance
#[derive(Default, Debug, Clone, Copy)]
struct Plane {
    normal: Vec3,
    d: f32,
}

impl Plane {
    pub fn from_vec4(v: Vec4) -> Self {
        let normal = v.truncate();
        let d = v.w;
        Plane { normal, d }
    }

    fn distance(&self, point: Vec3) -> f32 {
        self.normal.dot(point) + self.d
    }
}

pub struct Frustum {
    planes: [Plane; 6],
}

impl Frustum {
    pub fn from_world_projection(
        clip_from_world: &Mat4,
        view_translation: &Vec3,
        view_backward: &Vec3,
        far: f32,
    ) -> Self {
        let mut frustum = Frustum::from_world_projection_no_far(clip_from_world);
        let far_center = *view_translation - far * *view_backward;
        frustum.planes[5] = Plane::from_vec4(view_backward.extend(-view_backward.dot(far_center)));
        frustum
    }

    fn from_world_projection_no_far(clip_from_world: &Mat4) -> Self {
        let row3 = clip_from_world.row(3);
        let mut planes = [Plane::default(); 6];
        for (i, plane) in planes.iter_mut().enumerate().take(5) {
            let row = clip_from_world.row(i / 2);
            *plane = Plane::from_vec4(if (i & 1) == 0 && i != 4 {
                row3 + row
            } else {
                row3 - row
            });
        }

        Self { planes }
    }

    pub fn intersects_aabb(&self, aabb: &Bounds) -> bool {
        for plane in &self.planes {
            // Get the positive vertex in the direction of the plane normal
            let mut p_vertex = aabb.min;

            if plane.normal.x >= 0.0 {
                p_vertex.x = aabb.max.x;
            }
            if plane.normal.y >= 0.0 {
                p_vertex.y = aabb.max.y;
            }
            if plane.normal.z >= 0.0 {
                p_vertex.z = aabb.max.z;
            }

            // If the positive vertex is outside the plane, the whole AABB is outside
            if plane.distance(p_vertex) < 0.0 {
                return false; // No intersection
            }
        }

        true
    }

    pub fn intersects_sphere(&self, sphere: &Sphere) -> bool {
        for plane in &self.planes {
            // Calculate the distance from the sphere center to the plane
            let distance = plane.distance(sphere.center);

            // If the distance is less than the sphere radius, it intersects
            if distance < -sphere.radius {
                return false; // No intersection
            }
        }

        true // Intersects
    }
}
