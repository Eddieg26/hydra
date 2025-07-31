// const VERT_ID: AssetId<Shader> = AssetId::from_u128(0xabcdef0123456789);
// const VERT_ID_2: AssetId<Shader> = AssetId::from_u128(0x7fa18a3696e84df5848822a3b417e3f3u128);
// const DRAW_LIGHT_VERT: AssetId<Shader> = AssetId::from_u128(0x9e08450b1c394c8c88de79b6aa2c2589);
// const DRAW_LIGHT_FRAG: AssetId<Shader> = AssetId::from_u128(0x7503fc9b9eda4559a7fde1459f02058b);
// const FRAG_ID: AssetId<Shader> = AssetId::from_u128(0x123456789abcdef0);
// const FRAG_ID_2: AssetId<Shader> = AssetId::from_u128(0x9876543210fedcba);
// const FRAG_ID_3: AssetId<Shader> = AssetId::from_u128(0x3e7c2a1b4f5e4c2e9d1a8b7e6c5d4f3a);
// const UNLIT_WHITE: AssetId<UnlitColor> = AssetId::from_u128(0xfca61c1a76b14268b25058d36dbc6389);
// const UNLIT_RED: AssetId<UnlitColor> = AssetId::from_u128(0xa0cc79971c2d4206874539cb5ac54fe2u128);
// const UNLIT_BLUE: AssetId<UnlitColor> = AssetId::from_u128(0x87654321fedcba98);
// const LIT_WHITE: AssetId<LitColor> = AssetId::from_u128(0x9a8b7c6d5e4f3a2b1c0d8e7f6a5b4c3d);
// const LIT_RED: AssetId<LitColor> = AssetId::from_u128(0x1a2b3c4d5e6f708192a0b1c2d3e4f506u128);
// const LIGHT_MAT: AssetId<LightMaterial> = AssetId::from_u128(0xcd9c7e475e84435db8316d2612b94e2d);
// const QUAD_ID: AssetId<Mesh> = AssetId::from_u128(0xe51f72d138f747c6b22e2ac8a64b7b92u128);
// const CUBE_ID: AssetId<Mesh> = AssetId::from_u128(0x9d3919f428f8429a80e195849b3b6c21u128);
// const PLANE_ID: AssetId<Mesh> = AssetId::from_u128(0x2b3c4d5e6f708192a0b1c2d3e4f50607u128);
// const SWORD_ID: AssetId<Mesh> = AssetId::from_u128(0x6d3d79f5c6764b43993ae8de7ed0219bu128);
// const GENGAR_ID: AssetId<Texture> = AssetId::from_u128(0x43c5893d2b2f4a3bb2bb33eb1b362ff6u128);
// // const UNLIT_TEX_ID: AssetId<UnlitTexture> =
// //     AssetId::from_u128(0x1a2b3c4d5e6f708192a0b1c2d3e4f506u128);

// const QUAD: &[math::Vec2] = &[
//     math::Vec2::new(-0.5, -0.5), // Bottom-left
//     math::Vec2::new(0.5, -0.5),  // Bottom-right
//     math::Vec2::new(-0.5, 0.5),  // Top-left
//     math::Vec2::new(0.5, -0.5),  // Bottom-right
//     math::Vec2::new(0.5, 0.5),   // Top-right
//     math::Vec2::new(-0.5, 0.5),  // Top-left
// ];

// const QUAD_TEX_COORDS: &[math::Vec2] = &[
//     math::Vec2::new(0.0, 1.0), // Bottom-left
//     math::Vec2::new(1.0, 1.0), // Bottom-right
//     math::Vec2::new(0.0, 0.0), // Top-left
//     math::Vec2::new(1.0, 1.0), // Bottom-right
//     math::Vec2::new(1.0, 0.0), // Top-right
//     math::Vec2::new(0.0, 0.0), // Top-left
// ];

use asset::{Asset, AssetId, embed_asset, io::EmbeddedFs, plugin::AssetAppExt};
use ecs::{App, Component, Init, Spawner};
use math::{Mat4, Vec3};
use render::{
    AsBinding, Camera, Color, Drawable, Material, Mesh, MeshFilter, MeshSettings, Projection,
    RenderPhase, Renderer, Shader, ShaderSettings, ShaderType, Texture, View, plugin::RenderAppExt,
    wgpu::VertexFormat,
};
use transform::{GlobalTransform, Transform};

const COMMON_SHADER: AssetId<Shader> = AssetId::from_u128(0x3e7c2a1b4f5e4c2e9d1a8b7e6c5d4f3a);
const DRAW_MESH_SHADER: AssetId<Shader> = AssetId::from_u128(0xabcdef0123456789);
const UNLIT_COLOR_SHADER: AssetId<Shader> =
    AssetId::from_u128(0x7fa18a3696e84df5848822a3b417e3f3u128);
const UNLIT_TEX_SHADER: AssetId<Shader> = AssetId::from_u128(0x9e08450b1c394c8c88de79b6aa2c2589);
const CUBE: AssetId<Mesh> = AssetId::from_u128(0x123456789abcdef0);
const PLANE: AssetId<Mesh> = AssetId::from_u128(0xfca61c1a76b14268b25058d36dbc6389);
const UNLIT_WHITE: AssetId<UnlitColor> = AssetId::from_u128(0x9876543210fedcba);
const UNLIT_RED: AssetId<UnlitColor> = AssetId::from_u128(0x3e7c2a1b4f5e4c2e9d1a8b7e6c5d4f3a);

fn main() {
    let embedded = EmbeddedFs::new();
    embed_asset!(
        embedded,
        COMMON_SHADER,
        "shaders/common.wgsl",
        ShaderSettings::default()
    );

    embed_asset!(
        embedded,
        DRAW_MESH_SHADER,
        "shaders/draw-mesh.wgsl",
        ShaderSettings::default()
    );

    embed_asset!(
        embedded,
        UNLIT_COLOR_SHADER,
        "shaders/unlit-color.wgsl",
        ShaderSettings::default()
    );

    embed_asset!(
        embedded,
        UNLIT_TEX_SHADER,
        "shaders/unlit-texture.wgsl",
        ShaderSettings::default()
    );

    embed_asset!(embedded, CUBE, "meshes/cube.obj", MeshSettings::default());
    embed_asset!(embedded, PLANE, "meshes/plane.obj", MeshSettings::default());

    App::new()
        .add_drawable::<DrawMesh<UnlitColor>>()
        .add_source("embedded", embedded)
        .add_renderer::<Renderer3d>()
        .add_systems(Init, |mut spawner: Spawner| {
            spawner
                .spawn()
                .with_component(Camera::default())
                .with_component(View3d::default())
                .with_component(Transform::default().with_translation(Vec3::NEG_Z * 5.0))
                .with_component(GlobalTransform::default())
                .finish();

            spawner
                .spawn()
                .with_component(
                    Transform::default()
                        .with_rotation(Vec3::X * 90f32.to_radians())
                        .with_translation(Vec3::NEG_Y),
                )
                .with_component(GlobalTransform::default())
                .with_component(MeshFilter::from(PLANE))
                .with_component(DrawMesh::from(UNLIT_RED))
                .finish();

            spawner
                .spawn()
                .with_component(
                    Transform::default()
                        .with_scale(Vec3::splat(0.5))
                        .with_translation(Vec3::Y * 0.5),
                )
                .with_component(GlobalTransform::default())
                .with_component(MeshFilter::from(CUBE))
                .with_component(DrawMesh::from(UNLIT_WHITE))
                .finish();
        })
        .load_asset::<Mesh>(CUBE)
        .load_asset::<Mesh>(PLANE)
        .add_asset(UNLIT_WHITE, UnlitColor::from(Color::white()))
        .add_asset(UNLIT_RED, UnlitColor::from(Color::red()))
        .run();
}

#[derive(Clone, Component)]
pub struct View3d {
    projection: Projection,
}

impl Default for View3d {
    fn default() -> Self {
        Self {
            projection: Projection::Perspective {
                fov: 60.0f32.to_radians(),
                near: 0.1,
                far: 1000.0,
            },
        }
    }
}

impl View for View3d {
    type Transform = Transform;

    fn projection(&self, screen_size: math::Size) -> Mat4 {
        let aspect_ratio = screen_size.width / screen_size.height;
        match self.projection {
            Projection::Orthographic { near, far, size } => {
                let width = size * aspect_ratio;
                math::Mat4::orthographic_rh(-width, width, -size, size, near, far)
            }
            Projection::Perspective { fov, near, .. } => {
                math::Mat4::perspective_infinite_reverse_rh(fov, aspect_ratio, near)
            }
        }
    }

    fn far(&self) -> f32 {
        self.projection.far()
    }
}

#[derive(Clone, Copy, ShaderType)]
pub struct Model3d {
    world: Mat4,
}

#[derive(Component)]
pub struct DrawMesh<M: Material> {
    material: AssetId<M>,
}
impl<M: Material> From<AssetId<M>> for DrawMesh<M> {
    fn from(material: AssetId<M>) -> Self {
        Self { material }
    }
}

impl<M: Material> Clone for DrawMesh<M> {
    fn clone(&self) -> Self {
        Self {
            material: self.material,
        }
    }
}

impl<M> Drawable for DrawMesh<M>
where
    M: Material<View = View3d>,
{
    type View = View3d;

    type Material = M;

    fn material(&self) -> AssetId<Self::Material> {
        self.material
    }

    fn format() -> &'static [VertexFormat] {
        &[
            VertexFormat::Float32x3,
            VertexFormat::Float32x3,
            VertexFormat::Float32x2,
            VertexFormat::Float32x4,
        ]
    }

    fn shader() -> impl Into<AssetId<Shader>> {
        DRAW_MESH_SHADER
    }
}

#[derive(Default, Clone, Copy, PartialEq, PartialOrd)]
pub struct ZDistance(f32);

pub struct Opaque3d;
impl RenderPhase for Opaque3d {
    type View = View3d;

    type Item = ZDistance;

    fn mode() -> render::BlendMode {
        render::BlendMode::Opaque
    }
}

#[derive(Clone, Asset, AsBinding)]
pub struct UnlitColor {
    #[uniform(0)]
    color: Color,
}

impl Material for UnlitColor {
    type View = View3d;

    type Phase = Opaque3d;

    fn shader() -> impl Into<AssetId<render::Shader>> {
        UNLIT_COLOR_SHADER
    }
}

impl From<Color> for UnlitColor {
    fn from(color: Color) -> Self {
        Self { color }
    }
}

#[derive(Clone, Asset, AsBinding)]
pub struct UnlitTexture {
    #[texture(0)]
    texture: AssetId<Texture>,
}

impl Material for UnlitTexture {
    type View = View3d;

    type Phase = Opaque3d;

    fn shader() -> impl Into<AssetId<render::Shader>> {
        UNLIT_TEX_SHADER
    }
}

pub struct Renderer3d;
impl Renderer for Renderer3d {
    const NAME: render::Name = "Renderer3d";

    const CLEAR_MODE: render::ClearMode = render::ClearMode::Clear(Color::black());

    type Data = ();

    fn setup(_: &mut render::PassBuilder, phases: &mut render::RenderPhases) -> Self::Data {
        phases.add_phase::<Opaque3d>();
    }
}
