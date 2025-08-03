use asset::{Asset, AssetId, embed_asset, io::EmbeddedFs, plugin::AssetAppExt};
use ecs::{Component, Plugin, Resource, unlifetime::Read};
use math::Vec3;
use render::{
    AsBinding, BindGroup, BindGroupBuilder, BindGroupLayout, BindGroupLayoutBuilder, Color, Mesh,
    MeshSettings, Projection, RenderDevice, RenderResource, Shader, ShaderSettings, ShaderType,
    draw::{BlendMode, Drawable, Material, ShaderModel, ShaderModelData, ShaderPhase, Unlit, View},
    plugins::RenderAppExt,
    uniform::{UniformBuffer, UniformBufferArray},
    wgpu::ShaderStages,
};
use std::num::NonZero;
use transform::Transform;

pub const COMMON_SHADER: AssetId<Shader> = AssetId::from_u128(0x3e7c2a1b4f5e4c2e9d1a8b7e6c5d4f3a);
pub const DRAW_MESH_SHADER: AssetId<Shader> = AssetId::from_u128(0xabcdef0123456789);
pub const UNLIT_COLOR_SHADER: AssetId<Shader> =
    AssetId::from_u128(0x7fa18a3696e84df5848822a3b417e3f3u128);
pub const UNLIT_TEX_SHADER: AssetId<Shader> =
    AssetId::from_u128(0x9e08450b1c394c8c88de79b6aa2c2589);
pub const CUBE: AssetId<Mesh> = AssetId::from_u128(0x123456789abcdef0);
pub const PLANE: AssetId<Mesh> = AssetId::from_u128(0xfca61c1a76b14268b25058d36dbc6389);
pub const UNLIT_COLOR_MAT: AssetId<UnlitColor> =
    AssetId::from_u128(0xa0cc79971c2d4206874539cb5ac54fe2u128);

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

        embed_asset!(embedded, CUBE, "meshes/cube.obj", MeshSettings::default());
        embed_asset!(embedded, PLANE, "meshes/plane.obj", MeshSettings::default());

        app.add_drawable::<DrawMesh<UnlitColor>>()
            .add_source("embedded", embedded)
            .load_asset::<Mesh>(CUBE)
            .load_asset::<Mesh>(PLANE)
            .add_asset(UNLIT_COLOR_MAT, UnlitColor::from(Color::red()))
            .add_render_resource::<Lights>();
    }
}

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

    type Item = ZDistance;

    fn projection(&self, width: f32, height: f32) -> math::Mat4 {
        let aspect_ratio = width / height;
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

    fn shader() -> impl Into<AssetId<Shader>> {
        DRAW_MESH_SHADER
    }
}

pub struct Opaque3d;

impl ShaderPhase for Opaque3d {
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

    type Model = Unlit<ShaderModel3d>;

    type Phase = Opaque3d;

    fn shader() -> impl Into<AssetId<Shader>> {
        UNLIT_COLOR_SHADER
    }
}

pub struct ShaderModel3d {
    layout: BindGroupLayout,
    bind_group: BindGroup,
}

impl ShaderModel for ShaderModel3d {
    type Base = Self;

    type Data = ();

    type Arg = (Read<RenderDevice>, Option<Read<Lights>>);

    fn create((device, lights): ecs::ArgItem<Self::Arg>) -> Result<Self, render::ExtractError<()>> {
        let lights = lights.ok_or(render::ExtractError::Retry(()))?;

        let layout = BindGroupLayoutBuilder::new()
            .with_uniform(0, ShaderStages::FRAGMENT, false, None, None)
            .with_uniform(1, ShaderStages::FRAGMENT, false, None, None)
            .build(device);

        let bind_group = BindGroupBuilder::new(&layout)
            .with_uniform(0, lights.buffer.as_ref(), 0, None)
            .with_uniform(1, lights.count.as_ref(), 0, None)
            .build(device);

        Ok(Self { layout, bind_group })
    }

    fn bind_group(&self) -> Option<&BindGroup> {
        Some(&self.bind_group)
    }

    fn bind_group_layout(&self) -> Option<&BindGroupLayout> {
        Some(&self.layout)
    }

    fn setup(
        _: &mut render::PassBuilder,
        phases: &mut render::draw::ShaderPhases<Self>,
    ) -> Self::Data {
        phases.add_phase::<Opaque3d>();
    }
}

impl ShaderModel3d {
    pub fn update(model: &mut ShaderModelData<Self>, lights: &mut Lights, device: &RenderDevice) {
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
}

impl RenderResource for Lights {
    type Arg = Read<RenderDevice>;

    fn extract(device: ecs::ArgItem<Self::Arg>) -> Result<Self, render::ExtractError<()>> {
        let size = NonZero::new(Self::MAX_LIGHTS as u64 * LightData::min_size().get());
        let count = UniformBuffer::new(device, 0, None, None);
        let buffer = UniformBufferArray::with_size(
            device,
            size,
            LightData::min_size().get() as u32,
            None,
            None,
        );

        Ok(Self {
            buffer,
            count,
            max_size: size.unwrap(),
        })
    }
}

// const UNLIT_BLUE: AssetId<UnlitColor> = AssetId::from_u128(0x87654321fedcba98);
// const LIT_WHITE: AssetId<LitColor> = AssetId::from_u128(0x9a8b7c6d5e4f3a2b1c0d8e7f6a5b4c3d);
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
