use asset::{
    Asset, AssetEvent, AssetId, DefaultSettings, database::load::LoadError, embed_asset,
    importer::ImportError, io::EmbeddedFs, plugin::AssetAppExt,
};
use ecs::{App, Component, EventReader, Init, Spawner, Start};
use math::Vec3A;
use render::{
    AsBinding, BlendMode, Camera, Color, Draw, Material, Mesh, MeshAttribute, MeshAttributeType,
    MeshAttributeValues, MeshTopology, ObjImportSettings, Projection, RenderPhase, Renderer,
    Shader, ShaderSource, ShaderType, Texture, Texture2dSettings, View, ViewData,
    plugin::{RenderAppExt, RenderPlugin},
};
use transform::{GlobalTransform, Transform};

const VERT_ID: AssetId<Shader> = AssetId::from_u128(0xabcdef0123456789);
const VERT_ID_2: AssetId<Shader> = AssetId::from_u128(0x7fa18a3696e84df5848822a3b417e3f3u128);
const FRAG_ID: AssetId<Shader> = AssetId::from_u128(0x123456789abcdef0);
const FRAG_ID_2: AssetId<Shader> = AssetId::from_u128(0x9876543210fedcba);
const RED_MAT: AssetId<UnlitColor> = AssetId::from_u128(0xa0cc79971c2d4206874539cb5ac54fe2u128);
const BLUE_MAT: AssetId<UnlitColor> = AssetId::from_u128(0x87654321fedcba98);
const QUAD_ID: AssetId<Mesh> = AssetId::from_u128(0xe51f72d138f747c6b22e2ac8a64b7b92u128);
const CUBE_ID: AssetId<Mesh> = AssetId::from_u128(0x9d3919f428f8429a80e195849b3b6c21u128);
const SWORD_ID: AssetId<Mesh> = AssetId::from_u128(0x6d3d79f5c6764b43993ae8de7ed0219bu128);
const GENGAR_ID: AssetId<Texture> = AssetId::from_u128(0x43c5893d2b2f4a3bb2bb33eb1b362ff6u128);
// const UNLIT_TEX_ID: AssetId<UnlitTexture> =
//     AssetId::from_u128(0x1a2b3c4d5e6f708192a0b1c2d3e4f506u128);

const QUAD: &[math::Vec2] = &[
    math::Vec2::new(-0.5, -0.5), // Bottom-left
    math::Vec2::new(0.5, -0.5),  // Bottom-right
    math::Vec2::new(-0.5, 0.5),  // Top-left
    math::Vec2::new(0.5, -0.5),  // Bottom-right
    math::Vec2::new(0.5, 0.5),   // Top-right
    math::Vec2::new(-0.5, 0.5),  // Top-left
];

const QUAD_TEX_COORDS: &[math::Vec2] = &[
    math::Vec2::new(0.0, 1.0), // Bottom-left
    math::Vec2::new(1.0, 1.0), // Bottom-right
    math::Vec2::new(0.0, 0.0), // Top-left
    math::Vec2::new(1.0, 1.0), // Bottom-right
    math::Vec2::new(1.0, 0.0), // Top-right
    math::Vec2::new(0.0, 0.0), // Top-left
];

fn main() {
    let fs = EmbeddedFs::new();
    embed_asset!(fs, VERT_ID, "vert.wgsl", DefaultSettings::default());
    embed_asset!(fs, VERT_ID_2, "vert2.wgsl", DefaultSettings::default());
    embed_asset!(fs, FRAG_ID, "frag.wgsl", DefaultSettings::default());
    embed_asset!(fs, FRAG_ID_2, "frag2.wgsl", DefaultSettings::default());
    embed_asset!(fs, CUBE_ID, "cube.obj", ObjImportSettings::default());
    embed_asset!(fs, SWORD_ID, "sword.obj", ObjImportSettings::default());
    embed_asset!(fs, GENGAR_ID, "gengar.png", Texture2dSettings::default());

    let quad = Mesh::new(MeshTopology::TriangleList)
        .with_attribute(MeshAttribute::new(
            MeshAttributeType::Position,
            MeshAttributeValues::Vec2(QUAD.to_vec()),
        ))
        .with_attribute(MeshAttribute::new(
            MeshAttributeType::TexCoord0,
            MeshAttributeValues::Vec2(QUAD_TEX_COORDS.to_vec()),
        ));

    App::new()
        .add_plugins(RenderPlugin)
        .add_source("embedded", fs)
        .register_draw::<DrawMesh<UnlitColor>>()
        .add_renderer::<BasicRenderer>()
        .add_asset(RED_MAT, UnlitColor::from(Color::red()))
        .add_asset(BLUE_MAT, UnlitColor::from(Color::blue()))
        .add_asset(QUAD_ID, quad)
        .load_asset::<Mesh>(SWORD_ID)
        .load_asset::<Mesh>(CUBE_ID)
        .load_asset::<Texture>(GENGAR_ID)
        .add_systems(Init, |mut spawner: Spawner| {
            spawner
                .spawn()
                .with_component(GlobalTransform::with_translation(math::Vec3::Z * 5.0 + math::Vec3::X * 10.0))
                .with_component(Camera::default())
                .with_component(View3d::default())
                .finish();

            spawner
                .spawn()
                .with_component(GlobalTransform::with_translation(math::Vec3::X * 10.0))
                .with_component(Transform::default())
                .with_component(DrawMesh::<UnlitColor> {
                    material: RED_MAT,
                    mesh: SWORD_ID,
                })
                .finish();

            // spawner
            //     .spawn()
            //     .with_component(GlobalTransform::IDENTITY)
            //     .with_component(DrawMesh {
            //         material: BLUE_MAT,
            //         mesh: QUAD_ID,
            //     })
            //     .finish();
        })
        .add_systems(
            Start,
            |import_errors: EventReader<ImportError>,
             load_errors: EventReader<LoadError>,
             events: EventReader<AssetEvent<ShaderSource>>,
             texture_events: EventReader<AssetEvent<Texture>>,
             material_events: EventReader<AssetEvent<UnlitColor>>| {
                for error in import_errors {
                    println!("Import error: {}", error);
                }
                for error in load_errors {
                    println!("Load error: {}", error);
                }
                for event in events {
                    println!("Event: {:?}", event);
                }

                for event in texture_events {
                    println!("Event: {:?}", event);
                }

                for event in material_events {
                    println!("Event: {:?}", event);
                }
            },
        )
        .run();
}

pub struct Opaque3d;
impl RenderPhase for Opaque3d {
    fn mode() -> render::BlendMode {
        render::BlendMode::Opaque
    }
}

#[derive(Clone, Asset, AsBinding)]
#[uniform(0)]
pub struct UnlitColor {
    #[uniform]
    color: Color,
}

impl From<Color> for UnlitColor {
    fn from(color: Color) -> Self {
        Self { color }
    }
}

impl Material for UnlitColor {
    type Phase = Opaque3d;

    fn shader() -> impl Into<asset::AssetId<render::Shader>> {
        FRAG_ID
    }
}

// pub struct Transparent2d;
// impl RenderPhase for Transparent2d {
//     type Item = ();

//     fn mode() -> render::BlendMode {
//         render::BlendMode::Transparent
//     }
// }

// #[derive(Clone, Asset, AsBinding)]
// pub struct UnlitTexture {
//     #[texture(0, visibility = "fragment")]
//     #[sampler(1, visibility = "fragment")]
//     #[dependency]
//     texture: AssetId<Texture>,
// }

// impl From<AssetId<Texture>> for UnlitTexture {
//     fn from(texture: AssetId<Texture>) -> Self {
//         Self { texture }
//     }
// }

// impl Material for UnlitTexture {
//     type Phase = Transparent2d;

//     fn shader() -> impl Into<AssetId<Shader>> {
//         FRAG_ID_2
//     }
// }

#[derive(ShaderType)]
pub struct Mesh3d {
    world: math::Mat4,
}

#[derive(ShaderType)]
pub struct View3dData {
    world: math::Mat4,
    view: math::Mat4,
    projection: math::Mat4,
}

impl ViewData for View3dData {
    fn projection(&self) -> math::Mat4 {
        self.projection
    }
}

#[derive(Clone, Component)]
pub struct View3d {
    projection: Projection,
}

impl View3d {
    fn orthographic() -> Self {
        Self {
            projection: Projection::Orthographic {
                near: 0.01,
                far: 100.0,
                size: 1.0,
            },
        }
    }
}

impl Default for View3d {
    fn default() -> Self {
        Self {
            projection: Projection::Perspective {
                fov: 60.0f32.to_radians(),
                near: 0.01,
                far: 100.0,
            },
        }
    }
}

#[derive(Default, Clone, Copy, PartialEq, PartialOrd)]
pub struct ZDistance(f32);

impl View for View3d {
    type Data = View3dData;

    type Transform = Transform;

    type Item = ZDistance;

    fn data(&self, aspect_ratio: f32, _: &Camera, transform: &GlobalTransform) -> Self::Data {
        let projection = match self.projection {
            Projection::Orthographic { near, far, size } => {
                let width = size * aspect_ratio;
                math::Mat4::orthographic_rh(-width, width, -size, size, near, far)
            }
            Projection::Perspective { fov, near, .. } => {
                math::Mat4::perspective_infinite_reverse_rh(fov, aspect_ratio, near)
            }
        };

        let world = transform.matrix();
        let view = world.inverse();

        View3dData {
            world,
            view,
            projection,
        }
    }

    fn projection(&self) -> Projection {
        self.projection
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
}

#[derive(Clone, Component)]
pub struct DrawMesh<M: Material> {
    material: AssetId<M>,
    mesh: AssetId<Mesh>,
}

impl<M: Material> Draw for DrawMesh<M> {
    type View = View3d;

    type Model = Mesh3d;

    type Material = M;

    const CULL: bool = true;

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

    fn shader() -> impl Into<AssetId<render::Shader>> {
        VERT_ID
    }

    fn formats() -> &'static [render::wgpu::VertexFormat] {
        &[
            render::wgpu::VertexFormat::Float32x3,
            render::wgpu::VertexFormat::Float32x3,
            render::wgpu::VertexFormat::Float32x2,
            render::wgpu::VertexFormat::Float32x4,
        ]
    }
}

// #[derive(Clone, Component)]
// pub struct DrawSprite<M: Material> {
//     material: AssetId<M>,
//     mesh: AssetId<Mesh>,
// }

// impl<M: Material> Draw for DrawSprite<M> {
//     type View = View3d;

//     type Mesh = Mesh3d;

//     type Material = M;

//     fn material(&self) -> AssetId<Self::Material> {
//         self.material
//     }

//     fn mesh(&self) -> AssetId<Mesh> {
//         self.mesh
//     }

//     fn data(&self, transform: &GlobalTransform) -> Self::Mesh {
//         Mesh3d {
//             world: transform.matrix().to_cols_array(),
//         }
//     }

//     fn formats() -> &'static [render::wgpu::VertexFormat] {
//         &[
//             render::wgpu::VertexFormat::Float32x2,
//             render::wgpu::VertexFormat::Float32x2,
//         ]
//     }

//     fn shader() -> impl Into<AssetId<render::Shader>> {
//         VERT_ID_2
//     }
// }

pub struct BasicRenderer;

impl Renderer for BasicRenderer {
    type Data = ();

    const NAME: render::renderer::Name = "Basic Renderer";

    fn setup(_: &mut render::PassBuilder, phases: &mut render::RenderPhases) -> Self::Data {
        phases.add_phase::<View3d, Opaque3d>();
    }
}
