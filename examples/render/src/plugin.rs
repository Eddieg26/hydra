use asset::{Asset, AssetId, embed_asset, io::EmbeddedFs, plugin::AssetAppExt};
use ecs::{
    Component, IntoSystemConfig, Phase, Plugin, Query, Resource,
    app::sync::SyncComponentPlugin,
    system::{Always, Exists},
    unlifetime::Read,
};
use math::Vec3;
use render::{
    AsBinding, BindGroup, BindGroupBuilder, BindGroupLayout, BindGroupLayoutBuilder, Color, Mesh,
    MeshSettings, PostRender, PreRender, Projection, Queue, RenderApp, RenderDevice,
    RenderResource, Shader, ShaderSettings, ShaderType,
    constants::UniformBatchSize,
    draw::{BlendMode, Drawable, Material, RenderPhase, ShaderModel, Unlit, View},
    plugins::RenderAppExt,
    uniform::{UniformBuffer, UniformBufferArray},
    wgpu::ShaderStages,
};
use std::num::NonZero;
use transform::{GlobalTransform, Transform};

pub const COMMON_SHADER: AssetId<Shader> = AssetId::from_u128(0x3e7c2a1b4f5e4c2e9d1a8b7e6c5d4f3a);
pub const DRAW_MESH_SHADER: AssetId<Shader> = AssetId::from_u128(0xabcdef0123456789);
pub const UNLIT_COLOR_SHADER: AssetId<Shader> =
    AssetId::from_u128(0x7fa18a3696e84df5848822a3b417e3f3u128);
pub const UNLIT_TEX_SHADER: AssetId<Shader> =
    AssetId::from_u128(0x9e08450b1c394c8c88de79b6aa2c2589);
pub const LIT_COLOR_SHADER: AssetId<Shader> = AssetId::from_u128(0x87654321fedcba98);
pub const CUBE: AssetId<Mesh> = AssetId::from_u128(0x123456789abcdef0);
pub const PLANE: AssetId<Mesh> = AssetId::from_u128(0xfca61c1a76b14268b25058d36dbc6389);
pub const SPHERE: AssetId<Mesh> = AssetId::from_u128(0xe51f72d138f747c6b22e2ac8a64b7b92u128);
pub const SWORD: AssetId<Mesh> = AssetId::from_u128(0x6d3d79f5c6764b43993ae8de7ed0219bu128);
pub const UNLIT_COLOR_MAT: AssetId<UnlitColor> =
    AssetId::from_u128(0xa0cc79971c2d4206874539cb5ac54fe2u128);
pub const LIT_COLOR_WHITE: AssetId<LitColor> =
    AssetId::from_u128(0x9a8b7c6d5e4f3a2b1c0d8e7f6a5b4c3d);
pub const LIT_COLOR_RED: AssetId<LitColor> = AssetId::from_u128(0xcd9c7e475e84435db8316d2612b94e2d);
pub struct ExamplePlugin;

impl Plugin for ExamplePlugin {
    fn setup(&mut self, app: &mut ecs::AppBuilder) {
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

        embed_asset!(
            embedded,
            LIT_COLOR_SHADER,
            "shaders/forward-lighting.wgsl",
            ShaderSettings::default()
        );

        embed_asset!(embedded, CUBE, "meshes/cube.obj", MeshSettings::default());
        embed_asset!(embedded, PLANE, "meshes/plane.obj", MeshSettings::default());
        embed_asset!(embedded, SPHERE, "meshes/sphere.obj", MeshSettings::default());
        embed_asset!(embedded, SWORD, "meshes/sword.obj", MeshSettings::default());

        app.add_plugins(SyncComponentPlugin::<Light, RenderApp>::new())
            .add_drawable::<DrawMesh<UnlitColor>>()
            .add_drawable::<DrawMesh<LitColor>>()
            .add_source("embedded", embedded)
            // .load_asset::<Mesh>(CUBE)
            .load_asset::<Mesh>(PLANE)
            .load_asset::<Mesh>(SPHERE)
            .load_asset::<Mesh>(SWORD)
            .add_asset(UNLIT_COLOR_MAT, UnlitColor::from(Color::white()))
            .add_asset(LIT_COLOR_WHITE, LitColor::from(Color::white()))
            .add_asset(LIT_COLOR_RED, LitColor::from(Color::red()))
            .sub_app_mut(RenderApp)
            .add_sub_phase(Queue, QueueLights)
            .add_systems(PreRender, ShaderModel3d::update)
            .add_systems(QueueLights, Lights::queue)
            .add_systems(PostRender, Lights::clear);
    }

    fn build(&mut self, app: &mut ecs::AppBuilder) {
        let app = app.sub_app_mut(RenderApp);
        let lights = Lights::new(app.resource::<RenderDevice>());
        app.add_resource(lights);
    }
}

#[derive(Phase)]
pub struct QueueLights;

#[derive(Default, Clone, Copy, PartialEq, PartialOrd)]
pub struct ZDistance(f32);

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

    fn projection(&self, width: f32, height: f32) -> math::Mat4 {
        self.projection.matrix(width, height)
    }
}

#[derive(Component)]
pub struct DrawMesh<M: Material> {
    material: AssetId<M>,
}
impl<M: Material> From<AssetId<M>> for DrawMesh<M> {
    fn from(material: AssetId<M>) -> Self {
        DrawMesh { material }
    }
}

impl<M: Material> Clone for DrawMesh<M> {
    fn clone(&self) -> Self {
        Self {
            material: self.material,
        }
    }
}

impl<M: Material<View = View3d>> Drawable for DrawMesh<M> {
    type View = View3d;

    type Material = M;

    fn material(&self) -> AssetId<Self::Material> {
        self.material
    }

    fn vertex() -> &'static [render::wgpu::VertexFormat] {
        &[
            render::wgpu::VertexFormat::Float32x3,
            render::wgpu::VertexFormat::Float32x3,
            render::wgpu::VertexFormat::Float32x2,
            render::wgpu::VertexFormat::Float32x4,
        ]
    }

    fn primitive() -> render::wgpu::PrimitiveState {
        render::wgpu::PrimitiveState {
            cull_mode: Some(render::wgpu::Face::Back),
            ..Default::default()
        }
    }

    fn shader() -> impl Into<AssetId<Shader>> {
        DRAW_MESH_SHADER
    }
}

pub struct Opaque3d;

impl RenderPhase for Opaque3d {
    type View = View3d;

    fn mode() -> BlendMode {
        BlendMode::Opaque
    }
}

#[derive(Clone, Copy, Asset, AsBinding)]
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
    type View = View3d;

    type Model = Unlit;

    type Phase = Opaque3d;

    fn shader() -> impl Into<AssetId<Shader>> {
        UNLIT_COLOR_SHADER
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
    type View = View3d;

    type Model = ShaderModel3d;

    type Phase = Opaque3d;

    fn shader() -> impl Into<AssetId<Shader>> {
        LIT_COLOR_SHADER
    }
}

#[derive(Resource)]
pub struct ShaderModel3d {
    layout: BindGroupLayout,
    bind_group: BindGroup,
}

impl ShaderModel for ShaderModel3d {
    fn create(world: &ecs::World) -> Self {
        let device = world.resource::<RenderDevice>();
        let lights = world.resource::<Lights>();

        let layout = BindGroupLayoutBuilder::new()
            .with_uniform(0, ShaderStages::FRAGMENT, false, None, None)
            .with_uniform(1, ShaderStages::FRAGMENT, false, None, None)
            .build(device);

        let bind_group = BindGroupBuilder::new(&layout)
            .with_uniform(0, lights.buffer.as_ref(), 0, None)
            .with_uniform(1, lights.count.as_ref(), 0, None)
            .build(device);

        Self { layout, bind_group }
    }

    fn bind_group(&self) -> Option<&BindGroup> {
        Some(&self.bind_group)
    }

    fn layout(&self) -> Option<&BindGroupLayout> {
        Some(&self.layout)
    }
}

impl ShaderModel3d {
    pub fn update(model: &mut ShaderModel3d, lights: &mut Lights, device: &RenderDevice) {
        if let Some(_) = lights.update(device) {
            model.bind_group = BindGroupBuilder::new(&model.layout)
                .with_uniform(0, lights.buffer.as_ref(), 0, Some(lights.max_size))
                .with_uniform(1, lights.count.as_ref(), 0, None)
                .build(device);
        }
    }
}

#[derive(Clone, Copy, Component)]
pub struct Light {
    pub color: Color,
    pub intensity: f32,
    pub range: f32,
}

impl Light {
    pub fn with_color(mut self, color: Color) -> Self {
        self.color = color;
        self
    }

    pub fn with_intensity(mut self, intensity: f32) -> Self {
        self.intensity = intensity;
        self
    }

    pub fn with_range(mut self, range: f32) -> Self {
        self.range = range;
        self
    }
}

impl Default for Light {
    fn default() -> Self {
        Self {
            color: Color::white(),
            intensity: 1.0,
            range: 10.0,
        }
    }
}

#[derive(Clone, Copy, ShaderType)]
pub struct LightData {
    pub position: Vec3,
    pub range: f32,
    /// Alpha value for the light, used for attenuation.
    pub color: Color,
}

#[derive(Resource)]
pub struct Lights {
    buffer: UniformBufferArray<LightData>,
    count: UniformBuffer<u32>,
    max_size: NonZero<u64>,
}

impl Lights {
    pub const MAX_LIGHTS: u32 = 2048;

    pub fn new(device: &RenderDevice) -> Self {
        let max_lights = UniformBatchSize::<LightData>::size(device, Self::MAX_LIGHTS) as u64;
        let size = NonZero::new(max_lights * LightData::min_size().get());
        let count = UniformBuffer::new(device, 0, None, None);
        let buffer = UniformBufferArray::with_size(
            device,
            size,
            LightData::min_size().get() as u32,
            None,
            None,
        );

        Self {
            buffer,
            count,
            max_size: size.unwrap(),
        }
    }

    pub fn count(&self) -> &UniformBuffer<u32> {
        &self.count
    }

    pub fn push(&mut self, light: &LightData) {
        self.buffer.push(light);
    }

    fn update(&mut self, device: &RenderDevice) -> Option<u64> {
        self.count.set(self.buffer.len() as u32);
        self.count.update(device);

        self.buffer.update(device)
    }

    fn queue(lights: &mut Self, query: Query<(&Light, &GlobalTransform)>) {
        for (light, transform) in query.iter() {
            let Color { r, g, b, .. } = light.color;
            // let projection = Projection::Perspective {
            //     fov: 90.0,
            //     near: 0.1,
            //     far: 1000.0,
            // };

            // let view_proj_matrix = projection.matrix(2048f32, 2048f32) * transform.view_matrix();

            lights.buffer.push(&LightData {
                position: transform.translation(),
                range: light.range,
                color: Color::new(r, g, b, light.intensity),
            });
        }
    }

    fn clear(lights: &mut Self) {
        lights.buffer.clear();
    }
}

// const CUBE_ID: AssetId<Mesh> = AssetId::from_u128(0x9d3919f428f8429a80e195849b3b6c21u128);
// const PLANE_ID: AssetId<Mesh> = AssetId::from_u128(0x2b3c4d5e6f708192a0b1c2d3e4f50607u128);
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
