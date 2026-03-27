use asset::{AssetId, plugin::AssetAppExt};
use ecs::{App, AppBuilder, Init, Plugin, Resource, Spawner};
use renderer::plugin::RenderPlugin;
use renderer::{
    core::{ColorFormat, Msaa},
    output::OutputPass,
    plugin::RenderApp,
    renderer::{
        camera::{Camera, Projection, SettingState},
        graph::{
            GraphPass, Name, PassBuilder, RenderContext, RenderGraph, ResourceUsage, SurfaceDesc,
            SurfaceKind, SurfaceTexture, TextureSize,
        },
    },
    resources::{
        FragmentState, PipelineCache, PipelineId, RenderPipelineDesc, Shader, VertexState,
    },
    types::{Color, Viewport},
};
use std::borrow::Cow;
use wgpu::{
    ColorTargetState, MultisampleState, PrimitiveState, PushConstantRange, ShaderStages,
    TextureFormat,
};
use wgsl_macro::ShaderConstants;

/// Static asset ID for the clear shader.
const CLEAR_SHADER: AssetId<Shader> = AssetId::from_u128(0x00112233_4455_6677_8899_aabbccddeeff);

/// Resource holding the ClearPass pipeline ID.
#[derive(Resource)]
pub struct ClearPassPipeline(pub PipelineId);

pub struct SplitScreenPlugin;

impl Plugin for SplitScreenPlugin {
    fn setup(&mut self, app: &mut AppBuilder) {
        app.add_systems(Init, spawn_cameras);

        let clear_source = include_str!("../assets/shaders/clear.wgsl");
        app.add_asset(
            CLEAR_SHADER,
            Shader::new(clear_source, ShaderConstants::default(), false),
        );
    }

    fn finish(&mut self, app: &mut AppBuilder) {
        let render_app = app.sub_app_mut(RenderApp);

        let pipeline_id = render_app
            .resource_mut::<PipelineCache>()
            .queue_render_pipeline(RenderPipelineDesc {
                label: Some(Cow::Borrowed("clear_pass")),
                layout: vec![],
                vertex: VertexState {
                    shader: CLEAR_SHADER,
                    entry: Cow::Borrowed("vs_main"),
                    buffers: vec![],
                },
                fragment: Some(FragmentState {
                    shader: CLEAR_SHADER,
                    entry: Cow::Borrowed("fs_main"),
                    targets: vec![Some(ColorTargetState {
                        format: TextureFormat::Rgba16Float,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                primitive: PrimitiveState::default(),
                depth_stencil: None,
                multisample: MultisampleState::default(),
                push_constants: vec![PushConstantRange {
                    stages: ShaderStages::FRAGMENT,
                    range: 0..16,
                }],
            });

        render_app.add_resource(ClearPassPipeline(pipeline_id));

        let graph = render_app.resource_mut::<RenderGraph>();
        let output_pass_id = graph.add_pass::<OutputPass>();
        graph.add_before::<ClearPass>(output_pass_id);
    }
}

fn spawn_cameras(mut spawner: Spawner) {
    // Left camera: red, higher priority (renders first)
    spawner.spawn().with_component(Camera {
        projection: Projection::Orthographic {
            size: 1.0,
            near: 0.0,
            far: 1.0,
        },
        viewport: Viewport {
            x: 0.0,
            y: 0.0,
            width: 0.5,
            height: 1.0,
            depth: 0.0..1.0,
        },
        priority: 1,
        target: None,
        clear: Some(Color {
            r: 0.8,
            g: 0.2,
            b: 0.2,
            a: 1.0,
        }),
        msaa: SettingState::Auto,
    });

    // Right camera: blue, lower priority
    spawner.spawn().with_component(Camera {
        projection: Projection::Orthographic {
            size: 1.0,
            near: 0.0,
            far: 1.0,
        },
        viewport: Viewport {
            x: 0.5,
            y: 0.0,
            width: 0.5,
            height: 1.0,
            depth: 0.0..1.0,
        },
        priority: 0,
        target: None,
        clear: Some(Color {
            r: 0.2,
            g: 0.3,
            b: 0.8,
            a: 1.0,
        }),
        msaa: SettingState::Auto,
    });
}

pub struct ClearPass;

impl GraphPass for ClearPass {
    const NAME: Name = "ClearPass";

    fn setup(builder: &mut PassBuilder) -> impl Fn(&mut RenderContext) + Send + Sync + 'static {
        let surface = builder.create::<SurfaceTexture>(
            "intermediate_color",
            SurfaceDesc {
                size: TextureSize::Auto,
                kind: SurfaceKind::Color(ColorFormat::HDR),
                msaa: Msaa::Disabled,
            },
            ResourceUsage::Attachment,
        );

        builder.write(surface);

        move |ctx: &mut RenderContext<'_>| {
            let camera = ctx.camera().expect("ClearPass requires a camera");

            let Some(clear_color) = camera.clear else {
                return;
            };

            let width = camera.width as f32;
            let height = camera.height as f32;
            let clear_pipeline = ctx.world().resource::<ClearPassPipeline>();
            let pipeline_cache = ctx.world().resource::<PipelineCache>();
            let Some(pipeline) = pipeline_cache.get_render_pipeline(&clear_pipeline.0) else {
                return;
            };

            let surface = ctx.get(surface);
            let mut encoder = ctx.encoder("clear_pass");

            {
                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("clear_pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &surface.view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });

                pass.set_viewport(
                    camera.viewport.x * width,
                    camera.viewport.y * height,
                    camera.viewport.width * width,
                    camera.viewport.height * height,
                    camera.viewport.depth.start,
                    camera.viewport.depth.end,
                );

                let color = clear_color.as_slice();
                pass.set_pipeline(pipeline);
                pass.set_push_constants(ShaderStages::FRAGMENT, 0, bytemuck::cast_slice(&color));
                pass.draw(0..3, 0..1);
            }

            ctx.submit(encoder);
        }
    }
}

fn main() {
    App::new()
        .add_plugins((RenderPlugin, SplitScreenPlugin))
        .run();
}
