use ecs::{AppTag, Phase};

#[derive(AppTag, Default, Clone, Copy)]
pub struct RenderApp;

#[derive(Phase)]
pub struct Process;

#[derive(Phase)]
pub struct ProcessAssets;

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
