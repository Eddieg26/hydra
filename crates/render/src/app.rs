use ecs::{AppTag, Phase};
pub struct RenderApp;

impl AppTag for RenderApp {}

#[derive(Phase)]
pub struct PreProcess;

#[derive(Phase)]
pub struct Process;

#[derive(Phase)]
pub struct PostProcess;

#[derive(Phase)]
pub struct PreQueue;

#[derive(Phase)]
pub struct Queue;

#[derive(Phase)]
pub struct PreRender;

#[derive(Phase)]
pub struct Render;

#[derive(Phase)]
pub struct PostRender;

#[derive(Phase)]
pub struct Present;
