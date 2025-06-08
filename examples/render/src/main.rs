use asset::{Asset, AssetId};
use ecs::{App, Component};
use render::{
    AsBinding, Color, DepthOutput, Draw, DrawFunctions, GraphResourceId, Material, Mesh,
    RenderAssets, RenderMesh, RenderOutput, RenderPassDesc, RenderPhase, RenderState, Renderer,
    ShaderType, View, ViewBuffer, ViewDrawCalls,
    plugin::{RenderAppExt, RenderPlugin},
};
use transform::GlobalTransform;

fn main() {
    App::new()
        .add_plugins(RenderPlugin)
        .register_draw::<DrawMesh<UnlitColor>>()
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

impl Material for UnlitColor {
    type Phase = Opaque3d;

    fn shader() -> impl Into<asset::AssetId<render::Shader>> {
        AssetId::from_u128(0x123456789abcdef0)
    }
}

#[derive(ShaderType)]
pub struct Mesh3d {
    world: [f32; 16],
}

#[derive(ShaderType)]
pub struct View3dData {
    world: [f32; 16],
}

#[derive(Clone, Component)]
pub struct View3d;

impl View for View3d {
    type Data = View3dData;

    fn data(&self, transform: &GlobalTransform) -> Self::Data {
        View3dData {
            world: transform.matrix().to_cols_array(),
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
        &[render::wgpu::VertexFormat::Float32x3]
    }

    fn shader() -> impl Into<AssetId<render::Shader>> {
        AssetId::from_u128(0xabcdef0123456789)
    }
}

pub struct BasicRenderer;

impl Renderer for BasicRenderer {
    type Data = (GraphResourceId, GraphResourceId);

    fn setup(builder: &mut render::PassBuilder) -> Self::Data {
        let color = builder.write::<RenderOutput>();
        let depth = builder.write::<DepthOutput>();
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
                    load: wgpu::LoadOp::Clear(Color::red().into()),
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

    fn render<'a>(ctx: &mut render::RenderContext, pass: &'a mut render::wgpu::RenderPass<'a>) {
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

        let mut state = RenderState::new(pass);

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
