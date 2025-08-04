use crate::{
    RenderAssets, RenderContext, RenderDevice, RenderGraphError, RenderSurface,
    RenderSurfaceTexture, RenderTarget,
    primitives::{Color, Viewport},
    renderer::graph::SubGraph,
    resources::RenderTexture,
};
use asset::AssetId;
use ecs::{AddComponent, Commands, Component, Entity, Query, Resource, query::Without};
use encase::ShaderType;
use math::{Mat4, Size, Vec3, Vec3A, Vec4, bounds::Bounds, sphere::Sphere};

#[derive(Debug, Clone, Component, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Camera {
    pub viewport: Viewport,
    pub order: i32,
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

    pub fn matrix(&self, width: f32, height: f32) -> Mat4 {
        let aspect_ratio = width / height;
        match *self {
            Projection::Orthographic { near, far, size } => {
                let width = size * aspect_ratio;
                Mat4::orthographic_rh(-width, width, -size, size, near, far)
            }
            Projection::Perspective { fov, near, .. } => {
                Mat4::perspective_infinite_reverse_rh(fov, aspect_ratio, near)
            }
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

#[derive(Component)]
pub struct CameraAttachments {
    pub size: Size<u32>,
    pub clear: Option<Color>,
    pub color: Option<wgpu::TextureView>,
    pub depth: wgpu::TextureView,
    pub target: Option<AssetId<RenderTexture>>,
    pub viewport: Viewport,
}

impl CameraAttachments {
    pub(crate) fn queue(
        cameras: Query<(Entity, &Camera), Without<CameraAttachments>>,
        attachments: Query<(Entity, &Camera, &mut CameraAttachments)>,
        targets: &RenderAssets<RenderTarget>,
        device: &RenderDevice,
        surface: &RenderSurfaceTexture,
        sort_order: &mut CameraSortOrder,
        mut commands: Commands,
    ) {
        sort_order.clear();

        for (entity, camera) in cameras.iter() {
            let target = match camera.target {
                Some(id) => targets.get(&id).map(|t| (t.size(), t.view().clone())),
                None => surface.get().map(|s| {
                    (
                        Size::new(s.texture.width(), s.texture.height()),
                        s.texture.create_view(&Default::default()),
                    )
                }),
            };

            let Some((size, color)) = target else {
                continue;
            };

            let viewport = camera.viewport.scale(size.width as f32, size.height as f32);
            let depth = Self::create_depth_texture(device, size, RenderSurface::DEPTH_FORMAT);
            let attachments = CameraAttachments {
                size,
                clear: camera.clear_color,
                color: Some(color),
                depth: depth.create_view(&Default::default()),
                target: camera.target,
                viewport,
            };

            sort_order.push((entity, camera.order));
            commands.add(AddComponent::new(entity, attachments));
        }

        for (entity, camera, attachments) in attachments.iter() {
            let target = match camera.target {
                Some(id) => targets.get(&id).map(|t| (t.size(), t.view().clone())),
                None => surface.get().map(|s| {
                    (
                        Size::new(s.texture.width(), s.texture.height()),
                        s.texture.create_view(&Default::default()),
                    )
                }),
            };

            let Some((size, color)) = target else {
                attachments.color = None;
                continue;
            };

            attachments.color = Some(color);
            attachments.clear = camera.clear_color;
            attachments.viewport = camera.viewport.scale(size.width as f32, size.height as f32);

            if camera.target != attachments.target || size != attachments.size {
                let depth = Self::create_depth_texture(device, size, RenderSurface::DEPTH_FORMAT);
                attachments.depth = depth.create_view(&Default::default());
                attachments.size = size;
                attachments.target = camera.target;
            }

            sort_order.push((entity, camera.order));
        }

        sort_order.sort_by(|a, b| a.1.cmp(&b.1));
    }

    pub(crate) fn cleanup(cameras: Query<&mut CameraAttachments>) {
        for attachments in cameras.iter() {
            if attachments.target.is_none() {
                attachments.color = None;
            }
        }
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

#[derive(Default, Resource)]
pub struct CameraSortOrder(Vec<(Entity, i32)>);
impl std::ops::Deref for CameraSortOrder {
    type Target = Vec<(Entity, i32)>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for CameraSortOrder {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

pub struct CameraSubGraph;

impl CameraSubGraph {
    fn clear_screen<'a>(
        ctx: &mut RenderContext<'a>,
        attachments: &'a CameraAttachments,
    ) -> Option<()> {
        let color = attachments.color.as_ref()?;
        let color_attachments = vec![Some(wgpu::RenderPassColorAttachment {
            view: color,
            resolve_target: None,
            ops: wgpu::Operations {
                load: attachments
                    .clear
                    .map(|c| wgpu::LoadOp::Clear(c.into()))
                    .unwrap_or(wgpu::LoadOp::Load),
                store: wgpu::StoreOp::Store,
            },
        })];

        let depth_stencil_attachment = wgpu::RenderPassDepthStencilAttachment {
            view: &attachments.depth,
            depth_ops: Some(wgpu::Operations {
                load: wgpu::LoadOp::Clear(0f32),
                store: wgpu::StoreOp::Store,
            }),
            stencil_ops: None,
        };

        let mut encoder = ctx.encoder();
        encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("ClearPass"),
            color_attachments: &color_attachments,
            depth_stencil_attachment: Some(depth_stencil_attachment),
            timestamp_writes: Default::default(),
            occlusion_query_set: Default::default(),
        });

        Some(ctx.submit(encoder.finish()))
    }
}

impl SubGraph for CameraSubGraph {
    const NAME: super::graph::Name = "CameraSubGraph";

    fn run(ctx: &mut super::graph::RenderContext) -> Result<(), RenderGraphError> {
        let order = ctx.world().resource::<CameraSortOrder>();
        for (entity, _) in order.iter() {
            let Some(attachments) = ctx.world().get_component::<CameraAttachments>(*entity) else {
                continue;
            };

            Self::clear_screen(ctx, attachments);

            ctx.set_view(*entity, attachments);

            let _ = ctx.run_sub_graph(Self::NAME);
        }

        Ok(())
    }
}

/// A plane is represented as a 4D vector: ax + by + cz + d = 0
/// Where (a, b, c) is the normal and d is the distance
#[derive(Default, Debug, Clone, Copy, ShaderType)]
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

#[derive(Clone, Copy, ShaderType)]
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
