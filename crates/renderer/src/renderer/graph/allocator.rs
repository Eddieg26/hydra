use crate::{
    core::RenderDevice,
    renderer::graph::{RenderGraph, ResourceKind, ResourceNode},
    resources::{BindGroupBuilder, BindGroupLayoutBuilder, BindGroupLayoutRegistry, GpuResourceId},
};
use ecs::{FixedBitSet, IndexSet, World};
use std::{any::Any, collections::HashMap, hash::Hash};
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
    bind_groups: BindGroupCache,
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
            bind_groups: BindGroupCache::default(),
        }
    }

    pub fn update(&mut self, world: &World, device: &RenderDevice, graph: &RenderGraph) {}

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

    fn create_bind_group_cache(
        device: &RenderDevice,
        graph: &RenderGraph,
        allocations: &[GpuAllocation],
        layouts: &[BindGroupLayout],
        queue: IndexSet<BindGroupKey>,
    ) -> BindGroupCache {
        let mut cache = BindGroupCache::default();

        for key in queue {
            let BindGroupKey { layout, resources } = key;
            let mut builder = BindGroupBuilder::new();
            for id in &resources {
                let alloc = &allocations[*id as usize];
                let node = &graph.resources.nodes[alloc.node as usize];
                let ty = &graph.resources.types[node.ty as usize];
                ty.bind(&alloc.instance, &mut builder);
            }

            let archetype = cache.register(resources);
            let layout = &layouts[layout as usize];
            let bind_group = CachedBindGroup::new(layout.clone(), builder.build(device, layout));
            cache.add(archetype, bind_group);
        }

        cache.entries = (0..allocations.len()).map(|index| index as u32).collect();

        cache
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

pub struct CachedBindGroup {
    layout: BindGroupLayout,
    bind_group: BindGroup,
}

impl CachedBindGroup {
    pub fn new(layout: BindGroupLayout, bind_group: BindGroup) -> Self {
        Self { layout, bind_group }
    }
}

pub struct BindGroupArchetype {
    id: u32,
    bind_groups: Vec<GpuResourceId<BindGroup>>,
    allocations: FixedBitSet,
}

#[derive(Default)]
pub struct BindGroupCache {
    archetypes: Vec<BindGroupArchetype>,
    bind_groups: Vec<CachedBindGroup>,
    entries: Vec<u32>,
    map: HashMap<Box<[u32]>, u32>,
}

impl BindGroupCache {
    pub fn register(&mut self, resources: Box<[u32]>) -> u32 {
        let index = self.archetypes.len() as u32;
        let mut allocations = FixedBitSet::new();
        for resource in &resources {
            allocations.grow(*resource as usize);
            allocations.set(*resource as usize, true);
        }

        self.archetypes.push(BindGroupArchetype {
            id: index,
            bind_groups: Vec::new(),
            allocations,
        });

        self.map.insert(resources, index);

        index
    }

    pub fn update(
        &mut self,
        device: &RenderDevice,
        graph: &RenderGraph,
        allocations: &[GpuAllocation],
    ) {
        let mut updated = FixedBitSet::new();
        for index in 0..allocations.len() {
            let changed = allocations[index].generation != self.entries[index];
            self.entries[index] = allocations[index].generation;
            updated.grow(index);
            updated.set(index, changed);
        }

        if updated.is_empty() {
            return;
        }

        for archetype in &self.archetypes {
            if updated.is_disjoint(&archetype.allocations) {
                continue;
            }

            for id in &archetype.bind_groups {
                let cached = &mut self.bind_groups[id.get() as usize];
                let mut builder = BindGroupBuilder::new();
                for alloc in archetype.allocations.ones() {
                    let allocation = &allocations[alloc];
                    let node = &graph.resources.nodes[allocation.node as usize];
                    let ty = &graph.resources.types[node.ty as usize];
                    ty.bind(&allocation.instance, &mut builder);
                }

                cached.bind_group = builder.build(device, &cached.layout);
            }
        }
    }

    pub fn add(&mut self, archetype: u32, bind_group: CachedBindGroup) -> GpuResourceId<BindGroup> {
        let id = GpuResourceId::new(self.bind_groups.len() as u32);
        self.bind_groups.push(bind_group);
        self.archetypes[archetype as usize].bind_groups.push(id);

        id
    }
}
