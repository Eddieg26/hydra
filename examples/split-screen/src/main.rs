use asset::{AssetId, plugin::AssetAppExt};
use ecs::{
    AddComponent, App, AppBuilder, Commands, Component, Entity, Init, Plugin, Query,
    RemoveComponent, Resource, Spawner,
};
use renderer::core::RenderDevice;
use renderer::plugin::{Queue, RenderPlugin};
use renderer::renderer::camera::CameraSettings;
use renderer::renderer::graph::{GraphResource, ResourceAccess, ResourceKind};
use renderer::resources::{BindGroupLayoutRegistry, Buffer, BufferDesc, UniformArrayBuffer};
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
        BindGroupLayoutBuilder, FragmentState, PipelineCache, PipelineId, RenderPipelineDesc,
        Shader, VertexState,
    },
    types::{Color, Viewport},
};
use std::borrow::Cow;
use wgpu::BufferBindingType;
use wgpu::{
    BufferUsages, ColorTargetState, MultisampleState, PrimitiveState, ShaderStages, TextureFormat,
};
use wgsl_macro::ShaderConstants;

/// Static asset ID for the clear shader.
const CLEAR_SHADER: AssetId<Shader> = AssetId::from_u128(0x00112233_4455_6677_8899_aabbccddeeff);

/// Resource holding the ClearPass pipeline ID and bind group layout.
#[derive(Resource)]
pub struct ClearPassPipeline {
    pub pipeline: PipelineId,
}

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

        let device = render_app.resource::<RenderDevice>().clone();
        let bind_group_layout = {
            let registry = render_app.resource_mut::<BindGroupLayoutRegistry>();
            let mut builder = BindGroupLayoutBuilder::new();
            builder.with_label(Cow::Borrowed("clear_pass_bind_group_layout"));
            builder.with_uniform(ShaderStages::FRAGMENT, false, None, None);
            let id = registry.register(&device, builder);
            registry.get(id).clone()
        };

        let pipeline_id = render_app
            .resource_mut::<PipelineCache>()
            .queue_render_pipeline(RenderPipelineDesc {
                label: Some(Cow::Borrowed("clear_pass")),
                layout: vec![bind_group_layout.clone()],
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
                push_constants: vec![],
            });

        render_app.add_resource(ClearPassPipeline {
            pipeline: pipeline_id,
        });

        // let graph = render_app.resource_mut::<RenderGraph>();
        // let output_pass_id = graph.add_pass::<OutputPass>();
        // graph.add_before::<ClearPass>(output_pass_id);

        render_app.register::<ClearOffset>();
        render_app.add_systems(Queue, ClearColorBuffer::queue);
        render_app.add_resource(ClearColorBuffer::new(&device));
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

        builder.create::<ClearBuffer>(
            "clear_buffer",
            (),
            ResourceUsage::Binding {
                group: 0,
                binding: 0,
                visibility: ShaderStages::FRAGMENT,
                access: ResourceAccess::Read,
            },
        );

        move |ctx: &mut RenderContext<'_>| {
            let camera = ctx.camera().expect("ClearPass requires a camera");

            let Some(clear_offset) = ctx.world().get_component::<ClearOffset>(camera.entity) else {
                return;
            };

            let width = camera.width as f32;
            let height = camera.height as f32;
            let clear_pipeline = ctx.world().resource::<ClearPassPipeline>();
            let pipeline_cache = ctx.world().resource::<PipelineCache>();
            let Some(pipeline) = pipeline_cache.get_render_pipeline(&clear_pipeline.pipeline)
            else {
                return;
            };

            // Create a uniform buffer with the clear color data
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

                pass.set_pipeline(pipeline);
                pass.set_bind_group(0, ctx.bind_group(0), &[clear_offset.0]);
                pass.draw(0..3, 0..1);
            }

            ctx.submit(encoder);
        }
    }
}

#[derive(Resource)]
struct ClearColorBuffer {
    inner: UniformArrayBuffer<Color>,
    generation: u32,
}

impl ClearColorBuffer {
    pub fn new(device: &RenderDevice) -> Self {
        let inner = UniformArrayBuffer::new(
            device,
            BufferDesc {
                label: None,
                data: true,
                usages: BufferUsages::COPY_DST,
            },
        );

        Self {
            inner,
            generation: 0,
        }
    }

    pub fn push(&mut self, color: &Color) -> u32 {
        self.inner.push(color) as u32
    }

    pub fn clear(&mut self) {
        self.inner.clear();
    }

    pub fn update(&mut self, device: &RenderDevice) {
        if self.inner.update(device).is_some() {
            self.generation += 1;
        }
    }

    fn queue(
        buffer: &mut Self,
        device: &RenderDevice,
        cameras: Query<(Entity, &Camera, Option<&mut ClearOffset>)>,
        mut commands: Commands,
    ) {
        buffer.clear();

        for (entity, camera, offset) in cameras {
            let color = if let Some(color) = camera.clear {
                color
            } else if offset.is_some() {
                commands.add(RemoveComponent::<ClearOffset>::new(entity));
                continue;
            } else {
                continue;
            };

            if let Some(offset) = offset {
                offset.0 = buffer.push(&color);
            } else {
                let offset = ClearOffset(buffer.push(&color));
                commands.add(AddComponent::new(entity, offset));
            }
        }

        buffer.update(device);
    }
}

#[derive(Clone, Copy, Component)]
pub struct ClearOffset(u32);

struct ClearBuffer(Buffer);

impl GraphResource for ClearBuffer {
    type Desc = ();

    const OUTPUT: bool = true;

    fn resolve(
        _: &ecs::World,
        _: &renderer::core::RenderSettings,
        _: &mut renderer::renderer::graph::ResourceResolver,
        _: Self::Desc,
    ) -> Self::Desc {
        ()
    }

    fn create(world: &ecs::World, _: &RenderDevice, _: Name, _: &Self::Desc) -> Self {
        let buffer = world.resource::<ClearColorBuffer>();
        ClearBuffer(Buffer::clone(buffer.inner.as_ref()))
    }

    fn entry(
        _: &renderer::core::RenderSettings,
        _: &Self::Desc,
        builder: &mut BindGroupLayoutBuilder,
        visibility: ShaderStages,
    ) {
        builder.with_buffer(BufferBindingType::Uniform, visibility, true, None, None);
    }

    fn bind<'a>(&'a self, builder: &mut renderer::resources::BindGroupBuilder<'a>) {
        builder.with_buffer(self.0.as_entire_buffer_binding());
    }

    fn compatible(_: &Self::Desc, _: &Self::Desc) -> bool {
        true
    }

    fn generation(_: &Self::Desc, world: &ecs::World) -> u32 {
        let buffer = world.resource::<ClearColorBuffer>();
        buffer.generation
    }

    fn kind() -> renderer::renderer::graph::ResourceKind {
        ResourceKind::Imported
    }
}

fn main() {
    App::new()
        .add_plugins((RenderPlugin, SplitScreenPlugin))
        .run();
}
