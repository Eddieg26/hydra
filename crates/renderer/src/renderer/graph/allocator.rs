use crate::{
    core::RenderDevice,
    renderer::graph::{RenderGraph, ResourceKind, ResourceNode},
    resources::{BindGroupBuilder, BindGroupLayoutBuilder, BindGroupLayoutRegistry, GpuResourceId},
};
use ecs::{IndexSet, World};
use std::{any::Any, hash::Hash};
use wgpu::{BindGroup, BindGroupLayout, BindGroupLayoutEntry};

pub struct GpuAllocation {
    node: u32,
    generation: u32,
    instance: Box<dyn Any>,
}

pub struct GpuAllocationDesc {
    pub id: u32,
    pub node: u32,
    pub desc_hash: u64,
    pub kind: ResourceKind,
    pub last_use: Option<u32>,
}

impl GpuAllocationDesc {
    pub fn compatible(&self, node: &ResourceNode) -> bool {
        self.desc_hash == node.desc_hash
            && node.kind == ResourceKind::Transient
            && self.kind == ResourceKind::Transient
    }
}

pub struct GpuResourceAllocator {
    resources: Vec<u32>,
    allocations: Vec<GpuAllocation>,
    bind_groups: Vec<CachedBindGroup>,
}

impl GpuResourceAllocator {
    pub fn build(
        world: &World,
        device: &RenderDevice,
        graph: &RenderGraph,
        resources: Vec<u32>,
        mut descs: Vec<GpuAllocationDesc>,
    ) -> Self {
        let allocations = descs.drain(..).map(|desc| {
            let node = &graph.resources.nodes[desc.node as usize];
            let ty = &graph.resources.types[node.ty as usize];
            let instance = ty.create(world, device, node.name, &node.desc);
            GpuAllocation {
                node: node.id,
                generation: 0,
                instance,
            }
        });

        Self {
            resources,
            allocations: allocations.collect(),
            bind_groups: Vec::new(),
        }
    }

    fn create_layouts(
        device: &RenderDevice,
        layouts: &mut BindGroupLayoutRegistry,
        mut queue: IndexSet<Vec<BindGroupLayoutEntry>>,
    ) -> Vec<BindGroupLayout> {
        queue
            .drain(..)
            .map(|entries| {
                let id = layouts.register(device, BindGroupLayoutBuilder::from(entries));
                layouts.get(id).clone()
            })
            .collect()
    }

    fn create_bind_groups(
        device: &RenderDevice,
        graph: &RenderGraph,
        allocations: &[GpuAllocation],
        layouts: &[BindGroupLayout],
        mut queue: IndexSet<BindGroupKey>,
    ) -> Vec<CachedBindGroup> {
        queue
            .drain(..)
            .map(|key| {
                let mut builder = BindGroupBuilder::new();
                let mut entries = Vec::with_capacity(key.resources.len());
                for id in &key.resources {
                    let alloc = &allocations[*id as usize];
                    let node = &graph.resources.nodes[alloc.node as usize];
                    let ty = &graph.resources.types[node.ty as usize];
                    ty.bind(&alloc.instance, &mut builder);
                    entries.push(ResourceEntry {
                        allocation: *id,
                        generation: alloc.generation,
                    });
                }

                let layout = &layouts[key.layout as usize];
                let bind_group = builder.build(device, layout);
                CachedBindGroup {
                    layout: layout.clone(),
                    bind_group,
                    entries: entries.into_boxed_slice(),
                }
            })
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BindGroupKey {
    layout: u32,
    resources: Box<[u32]>,
}

impl BindGroupKey {
    pub fn new(layout: u32, resources: Box<[u32]>) -> Self {
        Self { layout, resources }
    }
}

impl Hash for BindGroupKey {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.layout.hash(state);
        self.resources.hash(state);
    }
}

#[derive(Default)]
pub struct ResourceEntry {
    allocation: u32,
    generation: u32,
}

pub struct CachedBindGroup {
    layout: BindGroupLayout,
    bind_group: BindGroup,
    entries: Box<[ResourceEntry]>,
}
