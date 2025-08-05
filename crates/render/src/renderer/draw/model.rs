use crate::{
    allocator::MeshAllocator, draw::{PhaseDrawCalls, View, ViewBuffer}, ActiveCamera, BindGroup, BindGroupLayout, Camera, CameraAttachments, ExtractError, PipelineCache, RenderCommandEncoder, RenderResource, RenderState
};
use ecs::{
    ArgItem, Entity, IntoSystemConfig, Phase, Query, ReadOnly, Resource, SystemArg, SystemConfig,
    World,
    query::{Single, With},
};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum BlendMode {
    Opaque,
    Transparent,
}

impl Into<wgpu::BlendState> for BlendMode {
    fn into(self) -> wgpu::BlendState {
        match self {
            BlendMode::Opaque => wgpu::BlendState::REPLACE,
            BlendMode::Transparent => wgpu::BlendState::ALPHA_BLENDING,
        }
    }
}

pub trait ShaderPhase: Send + Sync + 'static {
    type View: View;

    fn mode() -> BlendMode;
}

pub struct ShaderPhases<M: ShaderModel>(pub(crate) Vec<SystemConfig>, std::marker::PhantomData<M>);

impl<M: ShaderModel> ShaderPhases<M> {
    pub fn new() -> Self {
        Self(Vec::new(), std::marker::PhantomData)
    }

    pub fn add_phase<P: ShaderPhase>(&mut self) {
        let config = Self::run_shader_phase::<P>.config();
        self.0.push(config);
    }

    fn run_shader_phase<P: ShaderPhase>(
        view: Option<Single<(Entity, &CameraAttachments), (With<P::View>, With<ActiveCamera>)>>,
        multi: Query<Entity, With<Camera>>,
        views: &ViewBuffer<P::View>,
        draws: &PhaseDrawCalls<P, M>,
        meshes: &MeshAllocator,
        pipelines: &PipelineCache,
        model: Option<&ShaderModelData<M>>,
        world: &World,
        mut encoder: RenderCommandEncoder,
    ) {
        let Some((view, attachments)) = view.as_deref() else {
            return;
        };

        let Some(instance) = views.instance(view) else {
            return;
        };

        let Some(calls) = draws.0.get(&view) else {
            return;
        };

        let Some(color) = attachments.color.as_ref() else {
            return;
        };

        let mut color_attachments = vec![Some(wgpu::RenderPassColorAttachment {
            view: color,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Load,
                store: wgpu::StoreOp::Store,
            },
        })];

        color_attachments.extend(M::attachments());
        let depth_stencil_attachment = wgpu::RenderPassDepthStencilAttachment {
            view: &attachments.depth,
            depth_ops: Some(wgpu::Operations {
                load: wgpu::LoadOp::Load,
                store: wgpu::StoreOp::Store,
            }),
            stencil_ops: None,
        };

        let desc = wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &color_attachments,
            depth_stencil_attachment: Some(depth_stencil_attachment),
            timestamp_writes: Default::default(),
            occlusion_query_set: Default::default(),
        };

        let mut state = RenderState::new(encoder.begin_render_pass(&desc));

        for call in calls {
            let _ = call.draw(&mut state, instance, views, meshes, pipelines, model, world);
        }
    }
}

pub trait ShaderModel: Send + Sync + Sized + 'static {
    type Base: ShaderModel;

    type Arg: SystemArg + ReadOnly;

    fn create(arg: ArgItem<Self::Arg>) -> Result<Self, ExtractError<()>>;

    fn bind_group(&self) -> Option<&BindGroup> {
        None
    }

    fn bind_group_layout(&self) -> Option<&BindGroupLayout> {
        None
    }

    fn setup(phases: &mut ShaderPhases<Self>);

    fn attachments<'a>() -> Vec<Option<wgpu::RenderPassColorAttachment<'a>>> {
        vec![]
    }
}

#[derive(Resource)]
pub struct ShaderModelData<M: ShaderModel>(M);
impl<M: ShaderModel> std::ops::Deref for ShaderModelData<M> {
    type Target = M;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<M: ShaderModel> std::ops::DerefMut for ShaderModelData<M> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<M: ShaderModel> RenderResource for ShaderModelData<M> {
    type Arg = M::Arg;

    fn extract(arg: ecs::ArgItem<Self::Arg>) -> Result<Self, crate::ExtractError<()>> {
        M::create(arg).map(Self)
    }
}

impl ShaderModel for () {
    type Base = ();

    type Arg = ();

    fn create(_: ArgItem<Self::Arg>) -> Result<Self, crate::ExtractError<()>> {
        Ok(())
    }

    fn setup(_: &mut ShaderPhases<Self>) {}
}

pub struct Unlit<M: ShaderModel = ()>(std::marker::PhantomData<M>);
impl<M: ShaderModel> ShaderModel for Unlit<M> {
    type Base = M;

    type Arg = ();

    fn create(_: ArgItem<Self::Arg>) -> Result<Self, crate::ExtractError<()>> {
        Ok(Self(std::marker::PhantomData))
    }

    fn setup(_: &mut ShaderPhases<Self>) {}

    fn attachments<'a>() -> Vec<Option<wgpu::RenderPassColorAttachment<'a>>> {
        vec![]
    }
}

#[derive(Phase)]
pub struct DrawPass<M: ShaderModel>(std::marker::PhantomData<M>);
impl<M: ShaderModel> DrawPass<M> {
    pub(crate) fn new() -> Self {
        Self(std::marker::PhantomData)
    }
}

#[derive(Phase)]
pub struct MainDrawPass;
