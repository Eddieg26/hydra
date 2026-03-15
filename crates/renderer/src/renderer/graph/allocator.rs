use crate::{
    core::RenderDevice,
    renderer::graph::{RenderGraph, ResourceKind, ResourceVersion},
    resources::{BindGroupBuilder, BindGroupLayoutBuilder, BindGroupLayoutRegistry, GpuResourceId},
};
use ecs::{FixedBitSet, IndexSet, World};
use std::{any::Any, collections::HashMap, hash::Hash};
use wgpu::{BindGroup, BindGroupLayout, BindGroupLayoutEntry};

pub struct GpuAllocation {
    node: u32,
    generation: u32,
    instance: Box<dyn Any>,
    desc: Option<Box<dyn Any>>,
}

pub struct GpuAllocationDesc {
    pub id: u32,
    pub node: u32,
    pub key: u64,
    pub kind: ResourceKind,
    pub last_use: Option<u32>,
    pub desc: Option<Box<dyn Any>>,
}

impl GpuAllocationDesc {
    pub fn compatible(&self, instance: &ResourceVersion, kind: ResourceKind) -> bool {
        self.key == instance.key
            && kind == ResourceKind::Transient
            && self.kind == ResourceKind::Transient
    }
}

pub struct ImportedResource {
    alloc: u32,
    node: u32,
}

pub struct GpuResourceAllocator {
    imported: Vec<ImportedResource>,
    allocations: Vec<GpuAllocation>,
    bind_groups: BindGroupCache,
}

impl GpuResourceAllocator {
    pub fn build(
        world: &World,
        device: &RenderDevice,
        graph: &RenderGraph,
        mut descs: Vec<GpuAllocationDesc>,
    ) -> Self {
        let mut imported = Vec::new();
        let allocations = descs.drain(..).map(|alloc| {
            let node = &graph.resources.nodes[alloc.node as usize];
            let ty = &graph.resources.types[node.ty as usize];
            let desc = alloc.desc.as_ref().unwrap_or(&node.desc);
            let instance = ty.create(world, device, node.name, desc);
            let generation = match node.kind {
                ResourceKind::Imported => ty.generation(world, desc),
                ResourceKind::Transient => 0,
            };

            if node.kind == ResourceKind::Imported {
                imported.push(ImportedResource {
                    alloc: alloc.id,
                    node: node.id,
                });
            }

            GpuAllocation {
                node: node.id,
                generation,
                instance,
                desc: alloc.desc,
            }
        });

        Self {
            allocations: allocations.collect(),
            bind_groups: BindGroupCache::default(),
            imported,
        }
    }

    pub fn update(&mut self, world: &World, device: &RenderDevice, graph: &RenderGraph) {
        for index in 0..self.imported.len() {
            let ImportedResource { alloc, node } = self.imported[index];
            let node = &graph.resources.nodes[node as usize];
            let ty = &graph.resources.types[node.ty as usize];
            let desc = self.allocations[alloc as usize]
                .desc
                .as_ref()
                .unwrap_or(&node.desc);
            let generation = ty.generation(world, desc);
            self.allocations[alloc as usize].generation = generation;
        }

        let mut updated = FixedBitSet::new();
        for index in 0..self.allocations.len() {
            let changed = self.allocations[index].generation != self.bind_groups.entries[index];
            if changed {
                let allocation = &mut self.allocations[index];
                let node = &graph.resources.nodes[allocation.node as usize];
                let ty = &graph.resources.types[node.ty as usize];
                allocation.instance = ty.create(world, device, node.name, &node.desc);
                updated.grow(index);
                updated.set(index, true);
            }

            self.bind_groups.entries[index] = self.allocations[index].generation;
        }

        if !updated.is_empty() {
            self.bind_groups
                .update(updated, device, graph, &self.allocations);
        }
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
    fn build(
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

    pub fn register(&mut self, resources: Box<[u32]>) -> u32 {
        let index = self.archetypes.len() as u32;
        let mut allocations = FixedBitSet::new();
        for resource in &resources {
            allocations.grow(*resource as usize);
            allocations.set(*resource as usize, true);
        }

        self.archetypes.push(BindGroupArchetype {
            bind_groups: Vec::new(),
            allocations,
        });

        self.map.insert(resources, index);

        index
    }

    pub fn update(
        &mut self,
        updated: FixedBitSet,
        device: &RenderDevice,
        graph: &RenderGraph,
        allocations: &[GpuAllocation],
    ) {
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
}
