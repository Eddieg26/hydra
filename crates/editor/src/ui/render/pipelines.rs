use asset::AssetId;
use renderer::{
    core::{RenderDevice, RenderSettings},
    resources::{
        BindGroupLayoutBuilder, BindGroupLayoutRegistry, FragmentState, MeshAttributeType,
        PipelineCache, PipelineId, PositionType, RenderPipelineDesc, Shader, VertexBufferLayout,
        VertexState,
    },
    wgpu::{
        BlendState, ColorTargetState, ColorWrites, MultisampleState, PrimitiveState,
        SamplerBindingType, ShaderStages, TextureSampleType, TextureViewDimension,
    },
};

pub struct UiPipelines {
    solid: PipelineId,
    image: PipelineId,
    text: PipelineId,
}

impl UiPipelines {
    pub const SHADER: AssetId<Shader> = AssetId::from_u128(0);

    pub fn new(
        device: &RenderDevice,
        settings: &RenderSettings,
        registry: &mut BindGroupLayoutRegistry,
        cache: &mut PipelineCache,
    ) -> Self {
        let global_layout = registry.register(
            device,
            BindGroupLayoutBuilder::new().with_uniform(ShaderStages::VERTEX, false, None, None),
        );

        let texture_layout = registry.register(
            device,
            BindGroupLayoutBuilder::new()
                .with_texture(
                    ShaderStages::FRAGMENT,
                    TextureSampleType::Float { filterable: true },
                    TextureViewDimension::D2,
                    false,
                    None,
                )
                .with_sampler(ShaderStages::FRAGMENT, SamplerBindingType::Filtering, None),
        );

        let vertex_buffer_layout = VertexBufferLayout::from(vec![
            MeshAttributeType::Position {
                ty: PositionType::D2,
            },
            MeshAttributeType::UV0,
            MeshAttributeType::Index,
        ]);

        let primitive = PrimitiveState {
            topology: renderer::wgpu::PrimitiveTopology::TriangleList,
            strip_index_format: None,
            front_face: renderer::wgpu::FrontFace::Ccw,
            cull_mode: None,
            unclipped_depth: false,
            polygon_mode: renderer::wgpu::PolygonMode::Fill,
            conservative: false,
        };

        let multisample = MultisampleState {
            count: settings.msaa().sample_count(),
            mask: !0,
            alpha_to_coverage_enabled: false,
        };

        let solid = cache.queue_render_pipeline(RenderPipelineDesc {
            label: Some("ui_solid".into()),
            layout: vec![registry[global_layout].clone()],
            vertex: VertexState {
                shader: Self::SHADER,
                entry: "vs_main".into(),
                buffers: vec![vertex_buffer_layout.clone()],
            },
            fragment: Some(FragmentState {
                shader: Self::SHADER,
                entry: "fs_solid".into(),
                targets: vec![Some(ColorTargetState {
                    format: settings.color().into(),
                    blend: Some(BlendState::ALPHA_BLENDING),
                    write_mask: ColorWrites::ALL,
                })],
            }),
            primitive,
            depth_stencil: None,
            multisample,
            push_constants: vec![],
        });

        let image = cache.queue_render_pipeline(RenderPipelineDesc {
            label: Some("ui_image".into()),
            layout: vec![
                registry[global_layout].clone(),
                registry[texture_layout].clone(),
            ],
            vertex: VertexState {
                shader: Self::SHADER,
                entry: "vs_main".into(),
                buffers: vec![vertex_buffer_layout.clone()],
            },
            fragment: Some(FragmentState {
                shader: Self::SHADER,
                entry: "fs_image".into(),
                targets: vec![Some(ColorTargetState {
                    format: settings.color().into(),
                    blend: Some(BlendState::ALPHA_BLENDING),
                    write_mask: ColorWrites::ALL,
                })],
            }),
            primitive,
            depth_stencil: None,
            multisample,
            push_constants: vec![],
        });

        let text = cache.queue_render_pipeline(RenderPipelineDesc {
            label: Some("ui_text".into()),
            layout: vec![
                registry[global_layout].clone(),
                registry[texture_layout].clone(),
            ],
            vertex: VertexState {
                shader: Self::SHADER,
                entry: "vs_main".into(),
                buffers: vec![vertex_buffer_layout.clone()],
            },
            fragment: Some(FragmentState {
                shader: Self::SHADER,
                entry: "fs_text".into(),
                targets: vec![Some(ColorTargetState {
                    format: settings.color().into(),
                    blend: Some(BlendState::ALPHA_BLENDING),
                    write_mask: ColorWrites::ALL,
                })],
            }),
            primitive,
            depth_stencil: None,
            multisample,
            push_constants: vec![],
        });

        Self { solid, image, text }
    }

    pub fn solid(&self) -> PipelineId {
        self.solid
    }

    pub fn image(&self) -> PipelineId {
        self.image
    }

    pub fn text(&self) -> PipelineId {
        self.text
    }
}
