use ecs::{AppTag, Phase};
pub struct RenderApp;

impl AppTag for RenderApp {}

#[derive(Phase)]
pub struct Process;

#[derive(Phase)]
pub struct ProcessResources;

#[derive(Phase)]
pub struct ProcessAssets;

#[derive(Phase)]
pub struct ProcessPipelines;

#[derive(Phase)]
pub struct Queue;

#[derive(Phase)]
pub struct QueueViews;

#[derive(Phase)]
pub struct QueueDraws;

#[derive(Phase)]
pub struct PreRender;

#[derive(Phase)]
pub struct Render;

#[derive(Phase)]
pub struct PostRender;

#[derive(Phase)]
pub struct Present;
