use crate::{PassBuilder, RenderContext, SubGraph};
use std::{any::Any, collections::HashMap};

pub trait Renderer: Send + Sync + 'static {
    const NAME: super::Name;

    type Data: Send + Sync + 'static;

    type Base: Renderer;

    fn setup(builder: &mut PassBuilder) -> Self::Data;

    fn attachments<'a>(
        _ctx: &RenderContext<'a>,
        _data: &Self::Data,
    ) -> Vec<Option<wgpu::RenderPassColorAttachment<'a>>> {
        vec![]
    }
}

pub struct Unlit<R: Renderer>(std::marker::PhantomData<R>);
impl<R: Renderer> Renderer for Unlit<R> {
    const NAME: super::Name = R::NAME;

    type Data = ();

    type Base = R;

    fn setup(_: &mut PassBuilder) -> Self::Data {
        ()
    }
}

pub struct ErasedRenderer {
    setup: Box<dyn Fn(&mut PassBuilder) -> Box<dyn Any> + Send + Sync>,
    attachments: for<'a> fn(
        &RenderContext<'a>,
        &dyn Any,
    ) -> Vec<Option<wgpu::RenderPassColorAttachment<'a>>>,
}

impl ErasedRenderer {
    pub fn new<R: Renderer>() -> Self {
        Self {
            setup: Box::new(|builder| Box::new(R::setup(builder))),
            attachments: |ctx, data| {
                let data = data.downcast_ref::<R::Data>().unwrap();
                R::attachments(ctx, data)
            },
        }
    }
}

#[derive(Default)]
pub struct Renderers(pub(crate) HashMap<super::Name, ErasedRenderer>);

pub struct MainRenderPass {
    renderers: Vec<ErasedRenderer>,
}

impl SubGraph for MainRenderPass {
    const NAME: super::Name = "Main Render Pass";

    fn run(ctx: &mut RenderContext) -> Result<(), super::RenderGraphError> {
        ctx.run_sub_graph(Self::NAME)
    }
}
