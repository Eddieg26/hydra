use crate::{
    draw::{PhaseDrawCalls, View, ViewBuffer}, BindGroup, BindGroupLayout, GraphPass, PassBuilder, RenderContext, RenderGraphError, RenderState, SubGraph
};
use ecs::Entity;

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
            let views = ctx.world().resource::<ViewBuffer<M::View>>();
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

            let _ = calls
                .iter()
                .filter_map(|c| c.draw(state, view, ctx.world()).ok());
        };

        self.0.push(f)
    }
}

pub trait ShaderModel: Sized + 'static {
    type View: View;

    type Base: ShaderModel;

    type Data: Send + Sync + 'static;

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

pub struct Unlit<M: ShaderModel>(std::marker::PhantomData<M>);
impl<M: ShaderModel> ShaderModel for Unlit<M> {
    type View = M::View;

    type Base = M;

    type Data = ();

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

pub struct ClearPass;
impl GraphPass for ClearPass {
    const NAME: crate::Name = "Clear";

    fn setup(
        self,
        builder: &mut PassBuilder,
    ) -> impl Fn(&mut RenderContext) -> Result<(), RenderGraphError> + Send + Sync + 'static {
        builder.has_side_effect();

        |ctx| {
            let view = ctx.view().ok_or(RenderGraphError::MissingView)?;
            let color = view.target.color.as_ref();
            let color_attachments = vec![Some(wgpu::RenderPassColorAttachment {
                view: color
                    .as_ref()
                    .ok_or(RenderGraphError::MissingRenderTarget {
                        entity: view.entity,
                    })?,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(
                        view.target.clear.map_or(wgpu::Color::BLACK, |c| c.into()),
                    ),
                    store: wgpu::StoreOp::Store,
                },
            })];

            let depth_stencil_attachment = wgpu::RenderPassDepthStencilAttachment {
                view: &view.target.depth,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(0f32),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            };

            let mut encoder = ctx.encoder();
            encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("ClearPass"),
                color_attachments: &color_attachments,
                depth_stencil_attachment: Some(depth_stencil_attachment),
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

        move |ctx: &mut RenderContext| {
            let view = ctx.view().ok_or(RenderGraphError::MissingView)?;
            let color = view.target.color.as_ref();
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
                view: &view.target.depth,
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
