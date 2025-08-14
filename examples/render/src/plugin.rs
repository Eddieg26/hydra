// pub const COMMON_SHADER: AssetId<Shader> = AssetId::from_u128(0x3e7c2a1b4f5e4c2e9d1a8b7e6c5d4f3a);
// pub const DRAW_MESH_SHADER: AssetId<Shader> = AssetId::from_u128(0xabcdef0123456789);
// pub const UNLIT_COLOR_SHADER: AssetId<Shader> =
//     AssetId::from_u128(0x7fa18a3696e84df5848822a3b417e3f3u128);
// pub const UNLIT_TEX_SHADER: AssetId<Shader> =
//     AssetId::from_u128(0x9e08450b1c394c8c88de79b6aa2c2589);
// pub const LIT_COLOR_SHADER: AssetId<Shader> = AssetId::from_u128(0x87654321fedcba98);
// pub const CUBE: AssetId<Mesh> = AssetId::from_u128(0x123456789abcdef0);
// pub const PLANE: AssetId<Mesh> = AssetId::from_u128(0xfca61c1a76b14268b25058d36dbc6389);
// pub const SPHERE: AssetId<Mesh> = AssetId::from_u128(0xe51f72d138f747c6b22e2ac8a64b7b92u128);
// pub const SWORD: AssetId<Mesh> = AssetId::from_u128(0x6d3d79f5c6764b43993ae8de7ed0219bu128);
// pub const UNLIT_COLOR_MAT: AssetId<UnlitColor> =
//     AssetId::from_u128(0xa0cc79971c2d4206874539cb5ac54fe2u128);
// pub const LIT_COLOR_WHITE: AssetId<LitColor> =
//     AssetId::from_u128(0x9a8b7c6d5e4f3a2b1c0d8e7f6a5b4c3d);
// pub const LIT_COLOR_RED: AssetId<LitColor> = AssetId::from_u128(0xcd9c7e475e84435db8316d2612b94e2d);

const QUAD: AssetId<Mesh> = AssetId::from_u128(0x9d3919f428f8429a80e195849b3b6c21u128);
const DRAW_SPRITE_SHADER: AssetId<Shader> =
    AssetId::from_u128(0x2b3c4d5e6f708192a0b1c2d3e4f50607u128);
const GENGAR_ID: AssetId<Texture> = AssetId::from_u128(0x43c5893d2b2f4a3bb2bb33eb1b362ff6u128);
const MATERIAL_ID: AssetId<Material> = AssetId::from_u128(0x1a2b3c4d5e6f708192a0b1c2d3e4f506u128);

const QUAD_POSITIONS: &[Vec2] = &[
    Vec2::new(-0.5, -0.5), // Bottom-left
    Vec2::new(0.5, -0.5),  // Bottom-right
    Vec2::new(-0.5, 0.5),  // Top-left
    Vec2::new(0.5, -0.5),  // Bottom-right
    Vec2::new(0.5, 0.5),   // Top-right
    Vec2::new(-0.5, 0.5),  // Top-left
];

const QUAD_TEX_COORDS: &[Vec2] = &[
    Vec2::new(0.0, 1.0), // Bottom-left
    Vec2::new(1.0, 1.0), // Bottom-right
    Vec2::new(0.0, 0.0), // Top-left
    Vec2::new(1.0, 1.0), // Bottom-right
    Vec2::new(1.0, 0.0), // Top-right
    Vec2::new(0.0, 0.0), // Top-left
];

use asset::{Asset, AssetId, embed_asset, io::EmbeddedFs, plugin::AssetAppExt};
use ecs::{
    Component, Plugin, Resource,
    query::{Single, With},
    unlifetime::Read,
};
use math::{Mat4, Vec2};
use render::{
    ActiveCamera, AsBinding, BindGroup, BindGroupBuilder, BindGroupLayout, BindGroupLayoutBuilder,
    CameraAttachments, CameraPhase, FragmentState, Mesh, MeshAttribute, MeshAttributeValues,
    PipelineCache, PipelineId, Projection, RenderApp, RenderAsset, RenderAssets,
    RenderCommandEncoder, RenderDevice, RenderPipelineDesc, RenderState, RenderSurface, Shader,
    ShaderType, Texture, VertexState,
    allocator::MeshAllocator,
    plugins::{CameraPlugin, MeshPlugin, RenderPlugin, Texture2dPlugin, Texture2dSettings},
    storage::StorageBufferArray,
    uniform::UniformBufferArray,
    wgpu::{
        BlendState, ColorTargetState, ColorWrites, CompareFunction, DepthStencilState, Face,
        LoadOp, Operations, PrimitiveState, RenderPassColorAttachment,
        RenderPassDepthStencilAttachment, RenderPassDescriptor, SamplerBindingType, ShaderStages,
        StoreOp, TextureSampleType, TextureViewDimension,
    },
};
use transform::GlobalTransform;

pub struct ExamplePlugin;

impl Plugin for ExamplePlugin {
    fn setup(&mut self, app: &mut ecs::AppBuilder) {
        let assets = EmbeddedFs::new();

        embed_asset!(
            assets,
            GENGAR_ID,
            "textures/gengar.png",
            Texture2dSettings::default()
        );

        let quad = Mesh::new(render::MeshTopology::TriangleList)
            .with_attribute(MeshAttribute::new(
                render::MeshAttributeType::Position,
                MeshAttributeValues::Vec2(QUAD_POSITIONS.to_vec()),
            ))
            .with_attribute(MeshAttribute::new(
                render::MeshAttributeType::TexCoord0,
                MeshAttributeValues::Vec2(QUAD_TEX_COORDS.to_vec()),
            ));

        app.add_plugins((CameraPlugin, MeshPlugin, Texture2dPlugin))
            .add_source("example", assets)
            .load_asset::<Texture>(GENGAR_ID)
            .add_asset::<Mesh>(QUAD, quad)
            .sub_app_mut(RenderApp)
            .add_systems(
                CameraPhase,
                |camera: Single<(&GlobalTransform, &CameraAttachments), With<ActiveCamera>>,
                 surface: &RenderSurface,
                 pipelines: &PipelineCache,
                 meshes: &MeshAllocator,
                 materials: &RenderAssets<MaterialBinding>,
                 pipeline_data: &mut DrawPipeline,
                 mut encoder: RenderCommandEncoder| {
                    let Some(material) = materials.get(&MATERIAL_ID) else {
                        return;
                    };

                    let Some(pipeline) = pipelines.get_render_pipeline(&pipeline_data.id) else {
                        return;
                    };

                    let (transform, attachments) = *camera;

                    let Some(color) = attachments.color.as_ref() else {
                        return;
                    };

                    let Some(mesh) = meshes.vertex_slice(&QUAD) else {
                        return;
                    };

                    let data = ViewData::new(
                        transform,
                        &View2d {
                            near: 0.1,
                            far: 100.0,
                            size: 5.0,
                        },
                        surface.width() as f32,
                        surface.height() as f32,
                    );

                    let offset = pipeline_data.views.push(&data);
                    pipeline_data.objects.push(&Mat4::IDENTITY);

                    let pass = encoder.begin_render_pass(&RenderPassDescriptor {
                        label: None,
                        color_attachments: &vec![Some(RenderPassColorAttachment {
                            view: color,
                            resolve_target: None,
                            ops: Operations {
                                load: LoadOp::Load,
                                store: StoreOp::Store,
                            },
                        })],
                        depth_stencil_attachment: Some(RenderPassDepthStencilAttachment {
                            view: &attachments.depth,
                            depth_ops: Some(Operations {
                                load: LoadOp::Clear(1.0),
                                store: StoreOp::Store,
                            }),
                            stencil_ops: None,
                        }),
                        timestamp_writes: None,
                        occlusion_query_set: None,
                    });

                    let mut state = RenderState::new(pass);

                    state.set_pipeline(pipeline);
                    state.set_vertex_buffer(0, mesh.buffer.slice(..));
                    state.set_bind_group(0, &pipeline_data.view_binding, &[offset]);
                    state.set_bind_group(1, &pipeline_data.object_binding, &[]);
                    state.set_bind_group(2, &material.0, &[]);

                    state.draw(mesh.range, 0..1);
                },
            );
    }
}

pub struct View2d {
    near: f32,
    far: f32,
    size: f32,
}

#[derive(Clone, ShaderType)]
pub struct ViewData {
    view: Mat4,
    projection: Mat4,
}

impl ViewData {
    pub fn new(transform: &GlobalTransform, view: &View2d, width: f32, height: f32) -> Self {
        let projection = Projection::Orthographic {
            near: view.near,
            far: view.far,
            size: view.size,
        }
        .matrix(width, height);

        Self {
            view: transform.view_matrix(),
            projection,
        }
    }
}

#[derive(Resource)]
pub struct DrawPipeline {
    views: UniformBufferArray<ViewData>,
    objects: StorageBufferArray<Mat4>,

    view_binding: BindGroup,
    view_bind_group_layout: BindGroupLayout,

    object_binding: BindGroup,
    object_bind_group_layout: BindGroupLayout,

    material_bind_group_layout: BindGroupLayout,

    id: PipelineId,
}

impl DrawPipeline {
    pub fn new(
        device: &RenderDevice,
        surface: &RenderSurface,
        pipelines: &mut PipelineCache,
    ) -> Self {
        let views = UniformBufferArray::new(device, None, None);
        let objects = StorageBufferArray::new(device, None, None);

        let view_bind_group_layout = BindGroupLayoutBuilder::new()
            .with_uniform(0, ShaderStages::VERTEX, true, None, None)
            .build(device);

        let view_binding = BindGroupBuilder::new(&view_bind_group_layout)
            .with_uniform(0, views.as_ref(), 0, None)
            .build(device);

        let object_bind_group_layout = BindGroupLayoutBuilder::new()
            .with_storage(0, ShaderStages::VERTEX, false, true, None, None)
            .build(device);

        let object_binding = BindGroupBuilder::new(&object_bind_group_layout)
            .with_storage(0, objects.as_ref(), 0, None)
            .build(device);

        let material_bind_group_layout = BindGroupLayoutBuilder::new()
            .with_texture(
                0,
                ShaderStages::FRAGMENT,
                TextureViewDimension::D2,
                TextureSampleType::Float { filterable: false },
            )
            .with_sampler(1, ShaderStages::FRAGMENT, SamplerBindingType::NonFiltering)
            .build(device);

        let id = pipelines.queue_render_pipeline(RenderPipelineDesc {
            label: None,
            layout: vec![
                view_bind_group_layout.clone(),
                object_bind_group_layout.clone(),
                material_bind_group_layout.clone(),
            ],
            vertex: VertexState {
                shader: *DRAW_SPRITE_SHADER.as_ref(),
                entry: "vs_main".into(),
                buffers: vec![],
            },
            fragment: Some(FragmentState {
                shader: *DRAW_SPRITE_SHADER.as_ref(),
                entry: "fs_main".into(),
                targets: vec![Some(ColorTargetState {
                    format: surface.format(),
                    blend: Some(BlendState::REPLACE),
                    write_mask: ColorWrites::ALL,
                })],
            }),
            primitive: PrimitiveState {
                cull_mode: Some(Face::Back),
                ..Default::default()
            },
            depth_stencil: Some(DepthStencilState {
                format: surface.depth_format(),
                depth_write_enabled: true,
                depth_compare: CompareFunction::LessEqual,
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            push_constants: vec![],
        });

        Self {
            views,
            objects,
            view_binding,
            view_bind_group_layout,
            object_binding,
            object_bind_group_layout,
            material_bind_group_layout,
            id,
        }
    }
}

#[derive(Asset, Clone, AsBinding)]
pub struct Material {
    #[texture(0)]
    #[sampler(1)]
    texture: AssetId<Texture>,
}

pub struct MaterialBinding(BindGroup);

impl RenderAsset for MaterialBinding {
    type Source = Material;

    type Arg = (
        Read<RenderDevice>,
        Read<DrawPipeline>,
        <Material as AsBinding>::Arg,
    );

    fn extract(
        _: AssetId<Self::Source>,
        asset: Self::Source,
        (device, pipeline_data, arg): &mut ecs::ArgItem<Self::Arg>,
    ) -> Result<Self, render::ExtractError<Self::Source>> {
        let bind_group = asset
            .create_bind_group(device, &pipeline_data.material_bind_group_layout, arg)
            .map_err(|_| render::ExtractError::Retry(asset))?;
        Ok(Self(bind_group))
    }
}
