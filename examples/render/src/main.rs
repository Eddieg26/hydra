use asset::{
    Asset, AssetEvent, AssetId, DefaultSettings, database::load::LoadError, embed_asset,
    importer::ImportError, io::EmbeddedFs, plugin::AssetAppExt,
};
use ecs::{App, Component, Entity, EventReader, Init, Query, Spawner, Start, query::With};
use render::{
    AsBinding, Camera, CameraSubGraph, Color, DepthOutput, Draw, DrawFunctions, GraphResourceId,
    MainRenderPass, Material, Mesh, MeshAttribute, MeshAttributeType, MeshAttributeValues,
    MeshData, MeshTopology, Projection, RenderAssets, RenderMesh, RenderOutput, RenderPassDesc,
    RenderPhase, RenderState, Renderer, Shader, ShaderSource, ShaderType, View, ViewBuffer,
    ViewDrawCalls,
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
        .add_sub_graph_pass::<CameraSubGraph, MainRenderPass>(MainRenderPass::new(BasicRenderer))
        .add_asset(MAT_ID, UnlitColor::from(Color::red()), None)
        .add_asset(QUAD_ID, quad, None)
        .add_systems(Init, |mut spawner: Spawner| {
            spawner
                .spawn()
                .with_component(GlobalTransform::new(
                    math::Vec3::new(0.0, 0.0, -10.0),
                    math::Quat::IDENTITY,
                    math::Vec3::ONE,
                ))
                .with_component(Camera::default())
                .with_component(View3d::default())
                .finish();

            spawner
                .spawn()
                .with_component(GlobalTransform::IDENTITY)
                .with_component(DrawMesh::<UnlitColor> {
                    material: MAT_ID,
                    mesh: QUAD_ID,
                })
                .finish();
        })
        .add_systems(
            Start,
            |import_errors: EventReader<ImportError>,
             load_errors: EventReader<LoadError>,
             events: EventReader<AssetEvent<ShaderSource>>| {
                for error in import_errors {
                    println!("Import error: {}", error);
                }

                for error in load_errors {
                    println!("Load error: {}", error);
                }

                for event in events {
                    println!("Event: {:?}", event);
                }
            },
        )
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
    world: math::Mat4,
    view: math::Mat4,
    projection: math::Mat4,
}

#[derive(Clone, Component)]
pub struct View3d {
    projection: Projection,
}

impl Default for View3d {
    fn default() -> Self {
        Self {
            projection: Projection::Perspective {
                fov: 1.0,
                aspect_ratio: 1.0,
                near: 0.1,
                far: 100.0,
            },
        }
    }
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
            world,
            view,
            projection,
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
