use crate::{
    BindGroup, BindGroupLayout, Color, GraphPass, PassBuilder, RenderContext, RenderDevice,
    RenderGraphError, RenderResource, RenderState, SubGraph,
    draw::{PhaseDrawCalls, View, ViewBuffer},
};
use ecs::{Entity, Resource, unlifetime::Read};

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

pub type ShaderPhaseFn = for<'a> fn(Entity, &RenderContext<'a>, &mut RenderState<'a>);

pub struct ShaderPhases<M: ShaderModel>(Vec<ShaderPhaseFn>, std::marker::PhantomData<M>);

impl<M: ShaderModel> ShaderPhases<M> {
    pub fn new() -> Self {
        Self(Vec::new(), std::marker::PhantomData)
    }

    pub fn add_phase<P: ShaderPhase>(&mut self) {
        let f: ShaderPhaseFn = |entity, ctx, state| {
            let views = ctx.world().resource::<ViewBuffer<P::View>>();
            let Some(view) = views.instance(&entity) else {
                return;
            };

            let Some(calls) = ctx
                .world()
                .resource::<PhaseDrawCalls<P, M>>()
                .0
                .get(&entity)
            else {
                return;
            };

            for call in calls.iter() {
                let _ = call.draw(state, view, ctx.world());
            }
        };

        self.0.push(f)
    }
}

pub trait ShaderModel: Send + Sync + Sized + 'static {
    type Base: ShaderModel;

    type Data: Send + Sync + 'static;

    fn new(device: &RenderDevice) -> Self;

    fn bind_group(&self) -> Option<&BindGroup> {
        None
    }

    fn bind_group_layout(&self) -> Option<&BindGroupLayout> {
        None
    }

    fn setup(builder: &mut PassBuilder, phases: &mut ShaderPhases<Self>) -> Self::Data;

    fn attachments<'a>(
        _ctx: &RenderContext<'a>,
        _data: &Self::Data,
    ) -> Vec<Option<wgpu::RenderPassColorAttachment<'a>>> {
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
    type Arg = Read<RenderDevice>;

    fn extract(device: ecs::ArgItem<Self::Arg>) -> Result<Self, crate::ExtractError<()>> {
        Ok(Self(M::new(device)))
    }
}

impl ShaderModel for () {
    type Base = ();

    type Data = ();

    fn new(_: &RenderDevice) -> Self {
        ()
    }

    fn setup(_: &mut PassBuilder, _: &mut ShaderPhases<Self>) -> Self::Data {
        ()
    }
}

pub struct Unlit<M: ShaderModel = ()>(std::marker::PhantomData<M>);
impl<M: ShaderModel> ShaderModel for Unlit<M> {
    type Base = M;

    type Data = ();

    fn new(_: &RenderDevice) -> Self {
        Self(std::marker::PhantomData)
    }

    fn setup(_: &mut PassBuilder, _: &mut ShaderPhases<Self>) -> Self::Data {
        ()
    }

    fn attachments<'a>(
        _ctx: &RenderContext<'a>,
        _data: &Self::Data,
    ) -> Vec<Option<wgpu::RenderPassColorAttachment<'a>>> {
        vec![]
    }
}

#[derive(Debug, Default)]
pub struct ClearPass {
    pub color: Color,
}

impl From<Color> for ClearPass {
    fn from(color: Color) -> Self {
        Self { color }
    }
}

impl GraphPass for ClearPass {
    const NAME: crate::Name = "Clear";

    fn setup(
        self,
        builder: &mut PassBuilder,
    ) -> impl Fn(&mut RenderContext) -> Result<(), RenderGraphError> + Send + Sync + 'static {
        builder.has_side_effect();

        move |ctx| {
            let color = wgpu::RenderPassColorAttachment {
                view: ctx.surface_texture(),
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::from(self.color)),
                    store: wgpu::StoreOp::Store,
                },
            };

            let mut encoder = ctx.encoder();
            encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("ClearPass"),
                color_attachments: &vec![Some(color)],
                depth_stencil_attachment: None,
                timestamp_writes: Default::default(),
                occlusion_query_set: Default::default(),
            });

            Ok(ctx.submit(encoder.finish()))
        }
    }
}

pub struct DrawPass<M: ShaderModel>(std::marker::PhantomData<M>);

impl<M: ShaderModel> DrawPass<M> {
    pub fn new() -> Self {
        Self(std::marker::PhantomData)
    }
}

impl<M: ShaderModel> GraphPass for DrawPass<M> {
    const NAME: crate::Name = "DrawPass";

    fn setup(
        self,
        builder: &mut PassBuilder,
    ) -> impl Fn(&mut RenderContext) -> Result<(), crate::RenderGraphError> + Send + Sync + 'static
    {
        let mut phases = ShaderPhases::<M>::new();
        let name: &'static str = ecs::ext::short_type_name::<Self>();
        let data = M::setup(builder, &mut phases);
        builder.name = name;
        builder.has_side_effect();
        builder.dependency::<ClearPass>();

        move |ctx: &mut RenderContext| {
            let view = ctx.view().ok_or(RenderGraphError::MissingView)?;
            let color = view.attachments.color.as_ref();
            let mut encoder = ctx.encoder();
            let mut color_attachments = vec![Some(wgpu::RenderPassColorAttachment {
                view: color
                    .as_ref()
                    .ok_or(RenderGraphError::MissingRenderTarget {
                        entity: view.entity,
                    })?,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })];

            color_attachments.extend(M::attachments(ctx, &data));
            let depth_stencil_attachment = wgpu::RenderPassDepthStencilAttachment {
                view: &view.attachments.depth,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            };

            let desc = wgpu::RenderPassDescriptor {
                label: Some(name),
                color_attachments: &color_attachments,
                depth_stencil_attachment: Some(depth_stencil_attachment),
                timestamp_writes: Default::default(),
                occlusion_query_set: Default::default(),
            };

            let mut state = RenderState::new(encoder.begin_render_pass(&desc));
            for phase in &phases.0 {
                phase(view.entity, ctx, &mut state);
            }

            drop(state);

            Ok(ctx.submit(encoder.finish()))
        }
    }
}

pub struct MainDrawPass;

impl SubGraph for MainDrawPass {
    const NAME: crate::Name = "MainDrawPass";
}
