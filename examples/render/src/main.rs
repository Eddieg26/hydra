use asset::{Asset, AssetId, DefaultSettings, embed_asset, io::EmbeddedFs, plugin::AssetAppExt};
use ecs::{App, Component, Init, Spawner};
use render::{
    AsBinding, Color, DepthOutput, Draw, DrawFunctions, GraphResourceId, Material, Mesh,
    MeshAttribute, MeshAttributeType, MeshAttributeValues, MeshData, MeshTopology, Projection,
    RenderAssets, RenderMesh, RenderOutput, RenderPassDesc, RenderPhase, RenderState, Renderer,
    Shader, ShaderType, View, ViewBuffer, ViewDrawCalls,
    plugin::{RenderAppExt, RenderPlugin},
};
use transform::GlobalTransform;

const VERT_ID: AssetId<Shader> = AssetId::from_u128(0xabcdef0123456789);
const FRAG_ID: AssetId<Shader> = AssetId::from_u128(0x123456789abcdef0);
const MAT_ID: AssetId<UnlitColor> = AssetId::from_u128(0xdeadbeef12345678);
const QUAD_ID: AssetId<Mesh> = AssetId::from_u128(0xfeedface87654321);

const QUAD: &[math::Vec2] = &[
    math::Vec2::new(-0.5, -0.5), // Bottom-left
    math::Vec2::new(0.5, -0.5),  // Bottom-right
    math::Vec2::new(-0.5, 0.5),  // Top-left
    math::Vec2::new(0.5, -0.5),  // Bottom-right
    math::Vec2::new(0.5, 0.5),   // Top-right
    math::Vec2::new(-0.5, 0.5),  // Top-left
];

fn main() {
    let fs = EmbeddedFs::new();
    embed_asset!(fs, VERT_ID, "vert.wgsl", DefaultSettings::default());
    embed_asset!(fs, FRAG_ID, "frag.wgsl", DefaultSettings::default());

    let quad = Mesh::new(MeshTopology::TriangleList).with_attribute(MeshAttribute::new(
        MeshAttributeType::Position,
        MeshAttributeValues::Vec2(QUAD.to_vec()),
    ));

    App::new()
        .add_plugins(RenderPlugin)
        .add_source("embedded", fs)
        .register_draw::<DrawMesh<UnlitColor>>()
        .set_renderer(BasicRenderer)
        .add_asset(MAT_ID, UnlitColor::from(Color::red()), None)
        .add_asset(QUAD_ID, quad, None)
        .add_systems(Init, |mut spawner: Spawner| {
            spawner
                .spawn()
                .with_component(GlobalTransform::IDENTITY)
                .with_component(DrawMesh::<UnlitColor> {
                    material: MAT_ID,
                    mesh: QUAD_ID,
                })
                .finish();
        })
        .run();
}

pub struct Opaque3d;
impl RenderPhase for Opaque3d {
    type Item = ();

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

#[derive(ShaderType)]
pub struct Mesh3d {
    world: [f32; 16],
}

impl MeshData for Mesh3d {
    fn formats() -> &'static [render::wgpu::VertexFormat] {
        &[
            render::wgpu::VertexFormat::Float32x4,
            render::wgpu::VertexFormat::Float32x4,
            render::wgpu::VertexFormat::Float32x4,
            render::wgpu::VertexFormat::Float32x4,
        ]
    }
}

#[derive(ShaderType)]
pub struct View3dData {
    world: [f32; 16],
    view: [f32; 16],
    projection: [f32; 16],
}

#[derive(Clone, Component)]
pub struct View3d {
    projection: Projection,
}

impl View for View3d {
    type Data = View3dData;

    fn data(&self, transform: &GlobalTransform) -> Self::Data {
        let projection = match self.projection {
            Projection::Orthographic {
                left,
                right,
                bottom,
                top,
                near,
                far,
            } => math::Mat4::orthographic_rh(left, right, bottom, top, near, far),
            Projection::Perspective {
                fov,
                aspect_ratio,
                near,
                ..
            } => math::Mat4::perspective_infinite_reverse_rh(fov, aspect_ratio, near),
        };

        let world = transform.matrix();
        let view = world.inverse();

        View3dData {
            world: world.to_cols_array(),
            view: view.to_cols_array(),
            projection: projection.to_cols_array(),
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

    type Mesh = Mesh3d;

    type Material = M;

    fn material(&self) -> AssetId<Self::Material> {
        self.material
    }

    fn mesh(&self) -> AssetId<Mesh> {
        self.mesh
    }

    fn data(&self, transform: &GlobalTransform) -> Self::Mesh {
        Mesh3d {
            world: transform.matrix().to_cols_array(),
        }
    }

    fn formats() -> &'static [render::wgpu::VertexFormat] {
        &[render::wgpu::VertexFormat::Float32x2]
    }

    fn shader() -> impl Into<AssetId<render::Shader>> {
        VERT_ID
    }
}

pub struct BasicRenderer;

impl Renderer for BasicRenderer {
    type Data = (GraphResourceId, GraphResourceId);

    fn setup(builder: &mut render::PassBuilder) -> Self::Data {
        let color = builder.write::<RenderOutput>();
        let depth = builder.create::<DepthOutput>(());
        (color, depth)
    }

    fn build<'a>(ctx: &'a render::RenderContext<'a>, data: &'a Self::Data) -> RenderPassDesc<'a> {
        use render::wgpu;

        let (color, depth) = *data;

        let color = ctx.get::<RenderOutput>(color);
        let depth = ctx.get::<DepthOutput>(depth);

        RenderPassDesc {
            label: None,
            color_attachments: vec![Some(wgpu::RenderPassColorAttachment {
                view: color,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(Color::green().into()),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: depth,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(f32::MAX),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
        }
    }

    fn render<'a>(ctx: &mut render::RenderContext, mut state: RenderState<'a>) {
        let Some(camera) = ctx.camera() else {
            return;
        };

        let views = ctx.world().resource::<ViewBuffer<View3d>>();

        let Some(view) = views.get(camera.entity) else {
            return;
        };

        let meshes = ctx.world().resource::<RenderAssets<RenderMesh>>();
        let functions = ctx.world().resource::<DrawFunctions<View3d>>();
        let opaque = ctx.world().resource::<ViewDrawCalls<View3d, Opaque3d>>();

        opaque.draw(
            ctx,
            &mut state,
            &camera.entity,
            view,
            views,
            meshes,
            functions,
        );
    }
}
