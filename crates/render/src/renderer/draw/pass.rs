use crate::{
    CameraRenderTargets, Color, GraphPass, Name, PassBuilder, RenderContext, RenderGraphError,
    RenderState, drawable::PhaseDrawCalls, material::RenderPhase, view::ViewSet,
};
use ecs::Entity;

pub struct RenderPhases(
    Vec<(
        for<'a> fn(Entity, &RenderContext<'a>, &mut RenderState<'a>),
        usize,
    )>,
);

impl RenderPhases {
    pub fn add_phase<P: RenderPhase>(&mut self) {
        self.0.push((
            |entity, ctx, state| {
                let views = ctx.world().resource::<ViewSet<P::View>>();
                let Some(view) = views.0.get(&entity) else {
                    return;
                };

                let draw_calls = ctx.world().resource::<PhaseDrawCalls<P>>();
                let Some(calls) = draw_calls.views.get(&entity) else {
                    return;
                };

                for call in calls {
                    let _ = call.draw(state, view, ctx.world());
                }
            },
            P::mode() as usize,
        ));
    }

    fn render<'a>(&self, entity: Entity, ctx: &'a RenderContext<'a>, mut state: RenderState<'a>) {
        for phase in &self.0 {
            phase.0(entity, ctx, &mut state);
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum ClearMode {
    Clear(Color),
    Load,
}

pub trait Renderer: Send + Sync + 'static {
    const NAME: Name;

    const CLEAR_MODE: ClearMode = ClearMode::Load;

    type Data: Send + Sync + 'static;

    fn setup(builder: &mut PassBuilder, phases: &mut RenderPhases) -> Self::Data;

    fn attachments<'a>(
        _ctx: &RenderContext<'a>,
        _data: &Self::Data,
    ) -> Vec<Option<wgpu::RenderPassColorAttachment<'a>>> {
        vec![]
    }
}

pub struct DrawPass<R: Renderer>(std::marker::PhantomData<R>);
impl<R: Renderer> Default for DrawPass<R> {
    fn default() -> Self {
        Self(Default::default())
    }
}

impl<R: Renderer> GraphPass for DrawPass<R> {
    const NAME: Name = R::NAME;

    fn setup(
        self,
        builder: &mut PassBuilder,
    ) -> impl Fn(&mut RenderContext) -> Result<(), RenderGraphError> + Send + Sync + 'static {
        let mut phases = RenderPhases(Vec::new());
        let data = R::setup(builder, &mut phases);
        builder.has_side_effect();

        phases.0.sort_by_key(|p| p.1);

        move |ctx| {
            let view = ctx.view().ok_or(RenderGraphError::MissingView)?;
            let targets = ctx.world().resource::<CameraRenderTargets>();
            let target = targets
                .get(&view)
                .ok_or(RenderGraphError::MissingRenderTarget { entity: view })?;
            let color = target
                .color
                .as_ref()
                .ok_or(RenderGraphError::MissingRenderTarget { entity: view })?;

            let mut encoder = ctx.encoder();
            let mut color_attachments = vec![Some(wgpu::RenderPassColorAttachment {
                view: &color,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: match R::CLEAR_MODE {
                        ClearMode::Clear(color) => {
                            wgpu::LoadOp::Clear(target.clear.map_or(color.into(), |c| c.into()))
                        }
                        ClearMode::Load => wgpu::LoadOp::Load,
                    },
                    store: wgpu::StoreOp::Store,
                },
            })];

            color_attachments.extend(R::attachments(ctx, &data));
            let depth_stencil_attachment = wgpu::RenderPassDepthStencilAttachment {
                view: &target.depth,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(0f32),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            };

            let desc = wgpu::RenderPassDescriptor {
                label: Some(R::NAME),
                color_attachments: &color_attachments,
                depth_stencil_attachment: Some(depth_stencil_attachment),
                timestamp_writes: Default::default(),
                occlusion_query_set: Default::default(),
            };

            let state = RenderState::new(encoder.begin_render_pass(&desc));
            phases.render(view, ctx, state);
            Ok(ctx.submit(encoder.finish()))
        }
    }
}
