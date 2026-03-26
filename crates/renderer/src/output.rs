use crate::{
    core::{ColorFormat, Msaa},
    plugin::RenderApp,
    renderer::graph::{
        GraphPass, Name, PassBuilder, RenderContext, RenderGraph, RenderOutput, RenderOutputDesc,
        ResourceUsage, SurfaceDesc, SurfaceKind, SurfaceTexture, TextureSize,
    },
    resources::{
        BindGroupLayoutBuilder, BindGroupLayoutRegistry, FragmentState, PipelineCache, PipelineId,
        RenderPipelineDesc, Shader, TextureSampler, VertexState,
    },
};
use asset::{AssetId, plugin::AssetAppExt};
use ecs::{AppBuilder, Plugin, Resource};
use std::borrow::Cow;
use wgpu::{
    BindGroupLayout, ColorTargetState, MultisampleState, PrimitiveState, Sampler, ShaderStages,
    TextureFormat,
};
use wgsl_macro::ShaderConstants;

/// Static asset ID for the tonemap shader.
const TONEMAP_SHADER: AssetId<Shader> = AssetId::from_u128(0xa1b2c3d4_e5f6_7890_abcd_ef0123456789);

/// Static asset ID for the copy shader.
const COPY_SHADER: AssetId<Shader> = AssetId::from_u128(0xfedcba98_7654_3210_abcd_ef0123456789);

/// Resource holding the three pipeline IDs for the output pass.
#[derive(Resource)]
pub struct OutputPassPipelines {
    pub tonemap: PipelineId,
    pub tonemap_srgb: PipelineId,
    pub copy: PipelineId,
}

/// The pipeline action selected based on render settings and camera format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PipelineAction {
    Copy,
    Tonemap,
    TonemapSrgb,
    DirectCopy,
}

/// Selects the pipeline action based on the render settings color format
/// and the camera target format.
pub fn select_pipeline(settings_color: ColorFormat, camera_format: ColorFormat) -> PipelineAction {
    match (settings_color, camera_format) {
        (ColorFormat::Standard { .. }, ColorFormat::Standard { .. }) => PipelineAction::Copy,
        (ColorFormat::HDR, ColorFormat::Standard { srgb: false }) => PipelineAction::Tonemap,
        (ColorFormat::HDR, ColorFormat::Standard { srgb: true }) => PipelineAction::TonemapSrgb,
        (ColorFormat::HDR, ColorFormat::HDR) => PipelineAction::DirectCopy,
        (ColorFormat::Standard { .. }, ColorFormat::HDR) => unreachable!(),
    }
}

pub struct OutputPassPlugin;

impl Plugin for OutputPassPlugin {
    fn setup(&mut self, app: &mut AppBuilder) {
        let tonemap_source = include_str!("../assets/shaders/tonemap.wgsl");
        let copy_source = include_str!("../assets/shaders/copy.wgsl");

        app.add_asset(
            TONEMAP_SHADER,
            Shader::new(tonemap_source, ShaderConstants::default(), false),
        );
        app.add_asset(
            COPY_SHADER,
            Shader::new(copy_source, ShaderConstants::default(), false),
        );
    }

    fn finish(&mut self, app: &mut AppBuilder) {
        let render_app = app.sub_app_mut(RenderApp);

        // Create the bind group layout for the source texture (group 0, binding 0).
        let source_bgl = {
            let device = render_app.resource::<crate::core::RenderDevice>().clone();
            let registry = render_app.resource_mut::<BindGroupLayoutRegistry>();
            let mut builder = BindGroupLayoutBuilder::new();
            builder
                .with_texture(
                    ShaderStages::FRAGMENT,
                    wgpu::TextureSampleType::Float { filterable: true },
                    wgpu::TextureViewDimension::D2,
                    false,
                    None,
                )
                .with_sampler(
                    ShaderStages::FRAGMENT,
                    wgpu::SamplerBindingType::NonFiltering,
                    None,
                );

            let id = registry.register(&device, builder);
            registry.get(id).clone()
        };

        let pipelines = {
            let cache = render_app.resource_mut::<PipelineCache>();

            let tonemap_id = cache.queue_render_pipeline(make_pipeline_desc(
                TONEMAP_SHADER,
                TextureFormat::Bgra8Unorm,
                "output_tonemap",
                &source_bgl,
            ));

            let tonemap_srgb_id = cache.queue_render_pipeline(make_pipeline_desc(
                TONEMAP_SHADER,
                TextureFormat::Bgra8UnormSrgb,
                "output_tonemap_srgb",
                &source_bgl,
            ));

            let copy_id = cache.queue_render_pipeline(make_pipeline_desc(
                COPY_SHADER,
                TextureFormat::Bgra8Unorm,
                "output_copy",
                &source_bgl,
            ));

            OutputPassPipelines {
                tonemap: tonemap_id,
                tonemap_srgb: tonemap_srgb_id,
                copy: copy_id,
            }
        };

        render_app
            .add_resource(pipelines)
            .resource_mut::<RenderGraph>()
            .add_pass::<OutputPass>();
    }
}

fn make_pipeline_desc(
    shader: AssetId<Shader>,
    target_format: TextureFormat,
    label: &'static str,
    source_bgl: &BindGroupLayout,
) -> RenderPipelineDesc {
    RenderPipelineDesc {
        label: Some(Cow::Borrowed(label)),
        layout: vec![source_bgl.clone()],
        vertex: VertexState {
            shader,
            entry: Cow::Borrowed("vs_main"),
            buffers: vec![],
        },
        fragment: Some(FragmentState {
            shader,
            entry: Cow::Borrowed("fs_main"),
            targets: vec![Some(ColorTargetState {
                format: target_format,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: PrimitiveState::default(),
        depth_stencil: None,
        multisample: MultisampleState::default(),
        push_constants: vec![],
    }
}

pub struct OutputPass;

impl GraphPass for OutputPass {
    const NAME: Name = "OutputPass";

    fn setup(builder: &mut PassBuilder) -> impl Fn(&mut RenderContext) + Send + Sync + 'static {
        let color = builder.create::<SurfaceTexture>(
            "intermediate_color",
            SurfaceDesc {
                size: TextureSize::Auto,
                kind: SurfaceKind::Color(ColorFormat::HDR),
                msaa: Msaa::Disabled,
            },
        );
        let output = builder.create::<RenderOutput>("output", RenderOutputDesc::Auto);
        let sampler = builder.create::<Sampler>("default_sampler", TextureSampler::Default);

        let _src = builder.read(
            color,
            ResourceUsage::Binding {
                group: 0,
                binding: 0,
                visiblitiy: ShaderStages::FRAGMENT,
            },
        );
        let dst = builder.write(output, ResourceUsage::Attachment);
        let sampler = builder.read(
            sampler,
            ResourceUsage::Binding {
                group: 0,
                binding: 0,
                visiblitiy: ShaderStages::FRAGMENT,
            },
        );

        move |ctx: &mut RenderContext<'_>| {
            let camera = ctx.camera().expect("Missing camera for OutputPass");
            let action = select_pipeline(ctx.settings().color(), camera.format);

            let pipelines = ctx.world().resource::<OutputPassPipelines>();
            let pipeline_cache = ctx.world().resource::<PipelineCache>();

            let pipeline_id = match action {
                PipelineAction::Copy | PipelineAction::DirectCopy => &pipelines.copy,
                PipelineAction::Tonemap => &pipelines.tonemap,
                PipelineAction::TonemapSrgb => &pipelines.tonemap_srgb,
            };

            let Some(pipeline) = pipeline_cache.get_render_pipeline(pipeline_id) else {
                // Pipeline not yet compiled, skip this frame
                return;
            };

            let dst = ctx.get(dst);
            let mut encoder = ctx.encoder("output_pass");

            {
                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("output_pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &dst.view,
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

                pass.set_pipeline(pipeline);
                pass.set_bind_group(0, ctx.bind_group(0), &[]);
                pass.draw(0..3, 0..1);
            }

            ctx.submit(encoder);
        }
    }
}
