use asset::AssetId;
use ecs::Component;
use math::Mat4;
use render::{
    BlendMode, Camera, Draw, Material, Mesh, Projection, ShaderType, SubMesh, View, ViewData,
};
use transform::{GlobalTransform, Transform};

#[derive(Clone, Copy, ShaderType, Component)]
pub struct ViewData3d {
    pub world: Mat4,
    pub view: Mat4,
    pub projection: Mat4,
}

#[derive(Clone, Copy, Component)]
pub struct View3d {
    pub projection: Projection,
}

impl ViewData for ViewData3d {
    fn projection(&self) -> math::Mat4 {
        self.projection
    }
}

#[derive(Default, Clone, Copy, PartialEq, PartialOrd)]
pub struct ZDistance(f32);

impl View for View3d {
    type Data = ViewData3d;

    type Transform = Transform;

    type Item = ZDistance;

    fn data(&self, aspect_ratio: f32, _: &Camera, transform: &GlobalTransform) -> Self::Data {
        let projection = match self.projection {
            Projection::Orthographic { near, far, size } => {
                let width = size * aspect_ratio;
                Mat4::orthographic_rh(-width, width, -size, size, near, far)
            }
            Projection::Perspective { fov, near, .. } => {
                Mat4::perspective_infinite_reverse_rh(fov, aspect_ratio, near)
            }
        };

        let world = transform.matrix();
        let view = world.inverse();

        ViewData3d {
            world,
            view,
            projection,
        }
    }

    fn item(
        &self,
        data: &Self::Data,
        mode: BlendMode,
        _: &Self::Transform,
        global_transform: &GlobalTransform,
    ) -> Self::Item {
        let view_from_world = &data.view;
        let range_row = view_from_world.row(2);

        match mode {
            BlendMode::Opaque => ZDistance::default(),
            BlendMode::Transparent | BlendMode::Transmissive | BlendMode::AlphaMask => {
                let distance = range_row.dot(global_transform.matrix().row(3));
                ZDistance(distance)
            }
        }
    }

    fn projection(&self) -> Projection {
        self.projection
    }
}

#[derive(Clone, Copy, ShaderType)]
pub struct Mesh3d {
    pub world: Mat4,
}

#[derive(Clone, Component)]
pub struct DrawMesh<M: Material> {
    pub material: AssetId<M>,
    pub mesh: AssetId<Mesh>,
    pub sub_mesh: Option<AssetId<SubMesh>>,
}

impl<M: Material> DrawMesh<M> {
    pub fn new(material: AssetId<M>, mesh: AssetId<Mesh>) -> Self {
        Self {
            material,
            mesh,
            sub_mesh: None,
        }
    }

    pub fn with_sub_mesh(mut self, sub_mesh: AssetId<SubMesh>) -> Self {
        self.sub_mesh = Some(sub_mesh);
        self
    }
}

impl<M: Material> Draw for DrawMesh<M> {
    type Model = Mesh3d;

    type Material = M;

    fn material(&self) -> AssetId<Self::Material> {
        self.material
    }

    fn mesh(&self) -> AssetId<Mesh> {
        self.mesh
    }

    fn model(&self, transform: &GlobalTransform) -> Self::Model {
        Mesh3d {
            world: transform.matrix(),
        }
    }

    fn vertex() -> &'static [render::wgpu::VertexFormat] {
        &[]
    }

    fn shader() -> impl Into<AssetId<render::Shader>> {
        AssetId::from_u128(0)
    }
}
