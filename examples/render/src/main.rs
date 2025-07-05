use asset::{
    Asset, AssetEvent, AssetId, DefaultSettings, database::load::LoadError, embed_asset,
    importer::ImportError, io::EmbeddedFs, plugin::AssetAppExt,
};
use bytemuck::{Pod, Zeroable};
use ecs::{
    App, Component, EventReader, Extract, Init, IntoSystemConfig, Plugin, Resource, Spawner, Start,
    app::Main,
    system::Exists,
    unlifetime::{Read, SQuery},
};
use math::{Quat, Vec3};
use render::{
    ArrayBuffer, AsBinding, BindGroup, BindGroupBuilder, BindGroupLayout, BindGroupLayoutBuilder,
    BlendMode, Camera, Color, DisableCulling, Draw, Indices, Lighting, LightingData, Material,
    Mesh, MeshAttribute, MeshAttributeType, MeshAttributeValues, MeshTopology, ObjImportSettings,
    PostRender, Process, Projection, RenderApp, RenderDevice, RenderPhase, Renderer, Shader,
    ShaderSource, ShaderType, Texture, Texture2dSettings, Unlit, View, ViewData,
    plugin::{RenderAppExt, RenderPlugin},
    uniform::UniformBuffer,
    wgpu::{BufferUsages, ShaderStages},
};
use transform::{GlobalTransform, Transform};
use window::plugin::WindowPlugin;

const VERT_ID: AssetId<Shader> = AssetId::from_u128(0xabcdef0123456789);
const VERT_ID_2: AssetId<Shader> = AssetId::from_u128(0x7fa18a3696e84df5848822a3b417e3f3u128);
const DRAW_LIGHT_VERT: AssetId<Shader> = AssetId::from_u128(0x9e08450b1c394c8c88de79b6aa2c2589);
const DRAW_LIGHT_FRAG: AssetId<Shader> = AssetId::from_u128(0x7503fc9b9eda4559a7fde1459f02058b);
const FRAG_ID: AssetId<Shader> = AssetId::from_u128(0x123456789abcdef0);
const FRAG_ID_2: AssetId<Shader> = AssetId::from_u128(0x9876543210fedcba);
const FRAG_ID_3: AssetId<Shader> = AssetId::from_u128(0x3e7c2a1b4f5e4c2e9d1a8b7e6c5d4f3a);
const UNLIT_WHITE: AssetId<UnlitColor> = AssetId::from_u128(0xfca61c1a76b14268b25058d36dbc6389);
const UNLIT_RED: AssetId<UnlitColor> = AssetId::from_u128(0xa0cc79971c2d4206874539cb5ac54fe2u128);
const UNLIT_BLUE: AssetId<UnlitColor> = AssetId::from_u128(0x87654321fedcba98);
const LIT_WHITE: AssetId<LitColor> = AssetId::from_u128(0x9a8b7c6d5e4f3a2b1c0d8e7f6a5b4c3d);
const LIT_RED: AssetId<LitColor> = AssetId::from_u128(0x1a2b3c4d5e6f708192a0b1c2d3e4f506u128);
const LIGHT_MAT: AssetId<LightMaterial> = AssetId::from_u128(0xcd9c7e475e84435db8316d2612b94e2d);
const QUAD_ID: AssetId<Mesh> = AssetId::from_u128(0xe51f72d138f747c6b22e2ac8a64b7b92u128);
const CUBE_ID: AssetId<Mesh> = AssetId::from_u128(0x9d3919f428f8429a80e195849b3b6c21u128);
const PLANE_ID: AssetId<Mesh> = AssetId::from_u128(0x2b3c4d5e6f708192a0b1c2d3e4f50607u128);
const SWORD_ID: AssetId<Mesh> = AssetId::from_u128(0x6d3d79f5c6764b43993ae8de7ed0219bu128);
const WIRE_SPHERE_ID: AssetId<Mesh> = AssetId::from_u128(0x5e769e37d0b44dcbcc7029eb5b68320);
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
    embed_asset!(
        fs,
        FRAG_ID_3,
        "forward-lighting.wgsl",
        DefaultSettings::default()
    );
    embed_asset!(
        fs,
        DRAW_LIGHT_FRAG,
        "draw-light.frag.wgsl",
        DefaultSettings::default()
    );
    embed_asset!(
        fs,
        DRAW_LIGHT_VERT,
        "draw-light.vert.wgsl",
        DefaultSettings::default()
    );
    embed_asset!(fs, CUBE_ID, "cube.obj", ObjImportSettings::default());
    embed_asset!(
        fs,
        WIRE_SPHERE_ID,
        "sphere.obj",
        ObjImportSettings {
            wireframe: true,
            ..Default::default()
        }
    );
    embed_asset!(fs, SWORD_ID, "sword.obj", ObjImportSettings::default());
    embed_asset!(fs, PLANE_ID, "plane.obj", ObjImportSettings::default());
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
        .add_asset(UNLIT_WHITE, UnlitColor::from(Color::white()))
        .add_asset(UNLIT_RED, UnlitColor::from(Color::red()))
        .add_asset(UNLIT_BLUE, UnlitColor::from(Color::blue()))
        .add_asset(LIT_WHITE, LitColor::from(Color::white()))
        .add_asset(LIT_RED, LitColor::from(Color::red()))
        .add_asset(LIGHT_MAT, LightMaterial::from(Color::white()))
        .add_asset(QUAD_ID, quad)
        .load_asset::<Mesh>(SWORD_ID)
        .load_asset::<Mesh>(CUBE_ID)
        .load_asset::<Mesh>(PLANE_ID)
        .load_asset::<Mesh>(WIRE_SPHERE_ID)
        .load_asset::<Texture>(GENGAR_ID)
        .run();

    // App::new()
    //     .add_plugins(ForwardLightingPlugin)
    //     .add_source("embedded", fs)
    //     .register_draw::<DrawMesh<UnlitColor>>()
    //     .register_draw::<DrawMesh<LitColor>>()
    //     .register_draw::<Light>()
    //     .add_renderer::<BasicRenderer>()
    //     .add_asset(UNLIT_WHITE, UnlitColor::from(Color::white()))
    //     .add_asset(UNLIT_RED, UnlitColor::from(Color::red()))
    //     .add_asset(UNLIT_BLUE, UnlitColor::from(Color::blue()))
    //     .add_asset(LIT_WHITE, LitColor::from(Color::white()))
    //     .add_asset(LIT_RED, LitColor::from(Color::red()))
    //     .add_asset(LIGHT_MAT, LightMaterial::from(Color::white()))
    //     .add_asset(QUAD_ID, quad)
    //     .load_asset::<Mesh>(SWORD_ID)
    //     .load_asset::<Mesh>(CUBE_ID)
    //     .load_asset::<Mesh>(PLANE_ID)
    //     .load_asset::<Mesh>(WIRE_SPHERE_ID)
    //     .load_asset::<Texture>(GENGAR_ID)
    //     .add_systems(Init, |mut spawner: Spawner| {
    //         spawner
    //             .spawn()
    //             .with_component(GlobalTransform::with_translation(
    //                 math::Vec3::Z * 7.0 + math::Vec3::Y * 2.0,
    //             ))
    //             .with_component(Camera::default())
    //             .with_component(View3d::default())
    //             .finish();

    //         spawner
    //             .spawn()
    //             .with_component(GlobalTransform::with_translation(math::Vec3::Y * 2.0))
    //             .with_component(Transform::default())
    //             .with_component(Light::default())
    //             .finish();

    //         spawner
    //             .spawn()
    //             .with_component(GlobalTransform::ORIGIN)
    //             .with_component(Transform::default())
    //             .with_component(DrawMesh {
    //                 material: LIT_RED,
    //                 mesh: SWORD_ID,
    //             })
    //             .finish();

    //         spawner
    //             .spawn()
    //             .with_component(GlobalTransform::new(
    //                 Vec3::NEG_Y * 0.5,
    //                 Quat::from_euler(math::EulerRot::XYZ, -90.0f32.to_radians(), 0.0, 0.0),
    //                 Vec3::new(1.0, 1.0, 1.0),
    //             ))
    //             .with_component(Transform::default())
    //             .with_component(DrawMesh {
    //                 material: LIT_WHITE,
    //                 mesh: PLANE_ID,
    //             })
    //             .finish();
    //     })
    //     .add_systems(
    //         Start,
    //         |import_errors: EventReader<ImportError>,
    //          load_errors: EventReader<LoadError>,
    //          events: EventReader<AssetEvent<Mesh>>| {
    //             for error in import_errors {
    //                 println!("Import error: {}", error);
    //             }
    //             for error in load_errors {
    //                 println!("Load error: {}", error);
    //             }
    //             for event in events {
    //                 println!("Event: {:?}", event);
    //             }
    //         },
    //     )
    //     .run();
}

pub struct Opaque3d;
impl RenderPhase for Opaque3d {
    fn mode() -> render::BlendMode {
        render::BlendMode::Opaque
    }
}

#[derive(Clone, Asset, AsBinding)]
pub struct UnlitColor {
    #[uniform(0)]
    color: Color,
}

impl From<Color> for UnlitColor {
    fn from(color: Color) -> Self {
        Self { color }
    }
}

impl Material for UnlitColor {
    type Phase = Opaque3d;

    type Lighting = Unlit<View3d>;

    fn shader() -> impl Into<asset::AssetId<render::Shader>> {
        FRAG_ID
    }
}

#[derive(Clone, Asset, AsBinding)]
#[reload]
pub struct UnlitTexture {
    #[texture(0)]
    #[sampler(1)]
    #[dependency]
    texture: AssetId<Texture>,
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
    pub fn orthographic() -> Self {
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

    fn vertex() -> &'static [render::wgpu::VertexFormat] {
        &[
            render::wgpu::VertexFormat::Float32x3,
            render::wgpu::VertexFormat::Float32x3,
            render::wgpu::VertexFormat::Float32x2,
            render::wgpu::VertexFormat::Float32x4,
        ]
    }
}

pub struct BasicRenderer;

impl Renderer for BasicRenderer {
    type Data = ();

    const NAME: render::renderer::Name = "Basic Renderer";

    fn setup(_: &mut render::PassBuilder, phases: &mut render::RenderPhases) -> Self::Data {
        phases.add_phase::<View3d, Opaque3d>();
    }
}

#[derive(Clone, Copy, Component)]
pub struct Light {
    color: Color,
    range: f32,
    intensity: f32,
}

impl Default for Light {
    fn default() -> Self {
        Self {
            color: Color::white(),
            range: 10.0,
            intensity: 1.0,
        }
    }
}

#[derive(Clone, Copy, ShaderType, Pod, Zeroable)]
#[repr(C)]
pub struct LightData {
    position: Vec3,
    range: f32,
    color: Vec3,
    intensity: f32,
}

pub struct ForwardLighting {
    lights: ArrayBuffer<LightData>,
    light_count: UniformBuffer<u32>,
    layout: BindGroupLayout,
    binding: Option<BindGroup>,
}

impl ForwardLighting {
    pub fn new(device: &RenderDevice) -> Self {
        let lights = ArrayBuffer::new(100, BufferUsages::UNIFORM | BufferUsages::COPY_DST);
        let light_count = UniformBuffer::new(0);

        let layout = BindGroupLayoutBuilder::new()
            .with_storage(0, ShaderStages::FRAGMENT, false, true, None, None)
            .with_uniform(1, ShaderStages::FRAGMENT, false, None, None)
            .build(device);

        // let binding = BindGroupBuilder::new(&layout)
        //     .with_storage(0, &lights, 0, None)
        //     .with_uniform(1, &light_count, 0, None)
        //     .build(device);

        Self {
            lights,
            light_count,
            layout,
            binding: None,
        }
    }

    pub fn add_light(&mut self, light: &Light, position: Vec3) {
        self.lights.push(LightData {
            position,
            color: light.color.into(),
            range: light.range,
            intensity: light.intensity,
        });
    }

    pub fn update(&mut self, device: &RenderDevice) {
        self.light_count.set(self.lights.len() as u32);
        self.light_count.update(device);

        if let Some(lights) = self
            .lights
            .update(device)
            .and_then(|_| self.lights.buffer())
        {
            let binding = BindGroupBuilder::new(&self.layout)
                .with_storage(0, &lights, 0, None)
                .with_uniform(1, &self.light_count.buffer().unwrap(), 0, None)
                .build(device);

            self.binding = Some(binding);
        }
    }

    pub fn clear(&mut self) {
        self.lights.clear();
    }

    fn extract(
        lights: Main<SQuery<(&GlobalTransform, &Light)>>,
        lighting: &mut LightingData<Self>,
    ) {
        for (transform, light) in lights.iter() {
            lighting.add_light(light, transform.translation());
        }
    }

    fn process(lighting: &mut LightingData<Self>, device: &RenderDevice) {
        lighting.update(device);
    }

    fn clear_buffer(lighting: &mut LightingData<Self>) {
        lighting.clear();
    }
}

impl Lighting for ForwardLighting {
    type View = View3d;

    fn new(device: &RenderDevice) -> Self {
        Self::new(device)
    }

    fn bind_group_layout(&self) -> Option<&BindGroupLayout> {
        Some(&self.layout)
    }

    fn bind_group(&self) -> Option<&BindGroup> {
        self.binding.as_ref()
    }
}

#[derive(Clone, Copy, Asset, AsBinding)]
pub struct LitColor {
    #[uniform(0)]
    color: Color,
}

impl From<Color> for LitColor {
    fn from(color: Color) -> Self {
        Self { color }
    }
}

impl Material for LitColor {
    type Phase = Opaque3d;

    type Lighting = ForwardLighting;

    fn shader() -> impl Into<AssetId<Shader>> {
        FRAG_ID_3
    }
}

pub struct ForwardLightingPlugin;

impl Plugin for ForwardLightingPlugin {
    fn setup(&mut self, app: &mut ecs::AppBuilder) {
        let sub_app = app
            .add_plugins(RenderPlugin)
            .register::<Light>()
            .add_render_resource::<LightingData<ForwardLighting>>()
            .sub_app_mut(RenderApp)
            .unwrap();
        sub_app.add_systems(
            Extract,
            ForwardLighting::extract.when::<Exists<LightingData<ForwardLighting>>>(),
        );
        sub_app.add_systems(Process, ForwardLighting::process);
        sub_app.add_systems(PostRender, ForwardLighting::clear_buffer);
    }
}

#[derive(Clone, Asset, AsBinding)]
pub struct LightMaterial {
    #[uniform(0)]
    color: Color,
}

impl From<Color> for LightMaterial {
    fn from(color: Color) -> Self {
        Self { color }
    }
}

impl Material for LightMaterial {
    type Phase = Opaque3d;

    type Lighting = Unlit<View3d>;

    fn shader() -> impl Into<asset::AssetId<render::Shader>> {
        DRAW_LIGHT_FRAG
    }
}

impl Draw for Light {
    type Model = LightData;

    const CULL: bool = false;

    type Material = LightMaterial;

    fn material(&self) -> AssetId<Self::Material> {
        LIGHT_MAT
    }

    fn mesh(&self) -> AssetId<Mesh> {
        WIRE_SPHERE_ID
    }

    fn model(&self, transform: &GlobalTransform) -> Self::Model {
        LightData {
            position: transform.translation(),
            color: self.color.into(),
            range: self.range,
            intensity: self.intensity,
        }
    }

    fn primitive_state() -> render::wgpu::PrimitiveState {
        render::wgpu::PrimitiveState {
            topology: MeshTopology::TriangleList.into(),
            polygon_mode: render::wgpu::PolygonMode::Line,
            ..Default::default()
        }
    }

    fn vertex() -> &'static [render::wgpu::VertexFormat] {
        &[render::wgpu::VertexFormat::Float32x3]
    }

    fn shader() -> impl Into<AssetId<Shader>> {
        DRAW_LIGHT_VERT
    }
}
