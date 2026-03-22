use crate::{
    core::RenderDevice,
    renderer::graph::{
        BoxData, DynData, GraphEntryId, GraphResource, GraphResources, RenderGraph, ResourceKind,
    },
    resources::{BindGroupBuilder, BindGroupLayoutBuilder, BindGroupLayoutRegistry, GpuResourceId},
};
use ecs::{FixedBitSet, IndexSet};
use std::{collections::HashMap, hash::Hash};
use wgpu::{BindGroup, BindGroupLayout, BindGroupLayoutEntry};

pub struct GpuAllocation {
    node: u32,
    generation: u32,
    desc: BoxData,
    instance: BoxData,
}

impl GpuAllocation {
    pub fn instance(&self) -> &DynData {
        &self.instance
    }
}

pub struct GpuAllocationDesc {
    pub id: u32,
    pub node: u32,
    pub kind: ResourceKind,
    pub last_use: Option<u32>,
    pub desc: BoxData,
}

pub struct ImportedResource {
    alloc: u32,
    node: u32,
}

pub struct GpuResourceAllocator {
    resources: Box<[u32]>,
    imported: Box<[ImportedResource]>,
    allocations: Vec<GpuAllocation>,
    bind_groups: BindGroupCache,
}

impl GpuResourceAllocator {
    pub fn build(
        device: &RenderDevice,
        graph: &RenderGraph,
        resources: Box<[u32]>,
        descs: Vec<GpuAllocationDesc>,
        layouts: Vec<BindGroupLayout>,
        bind_groups: IndexSet<BindGroupKey>,
    ) -> Self {
        let mut imported = Vec::new();
        let mut allocations = Vec::with_capacity(descs.len());
        for alloc in descs {
            let node = &graph.resources.nodes[alloc.node as usize];
            let ty = &graph.resources.types[node.ty as usize];
            let instance = ty.create(device, node.name, &alloc.desc);

            if node.kind == ResourceKind::Imported {
                imported.push(ImportedResource {
                    alloc: alloc.id,
                    node: node.id,
                });
            }

            allocations.push(GpuAllocation {
                node: node.id,
                desc: alloc.desc,
                generation: ty.generation(&instance),
                instance,
            });
        }

        let bind_groups = BindGroupCache::build(device, graph, &allocations, &layouts, bind_groups);

        Self {
            resources,
            imported: imported.into_boxed_slice(),
            allocations,
            bind_groups,
        }
    }

    pub fn allocations(&self) -> &[GpuAllocation] {
        &self.allocations
    }

    pub fn bind_groups(&self) -> &BindGroupCache {
        &self.bind_groups
    }

    pub fn resource<R: GraphResource>(&self, id: GraphEntryId<R>) -> &R {
        let index = self.resources[id.0 as usize];
        self.allocations[index as usize]
            .instance
            .downcast_ref::<R>()
            .unwrap()
    }

    pub fn update(&mut self, device: &RenderDevice, resources: &GraphResources) {
        for index in 0..self.imported.len() {
            let ImportedResource { alloc, node } = self.imported[index];
            let generation = {
                let resource = &self.allocations[alloc as usize].instance;
                let node = &resources.nodes[node as usize];
                let ty = &resources.types[node.ty as usize];
                ty.generation(resource)
            };
            self.allocations[alloc as usize].generation = generation;
        }

        let mut updated = FixedBitSet::new();
        for index in 0..self.allocations.len() {
            let changed = self.allocations[index].generation != self.bind_groups.entries[index];
            if changed {
                let allocation = &mut self.allocations[index];
                let node = &resources.nodes[allocation.node as usize];
                let ty = &resources.types[node.ty as usize];
                allocation.instance = ty.create(device, node.name, &allocation.desc);
            }

            self.bind_groups.entries[index] = self.allocations[index].generation;
            updated.grow(index);
            updated.set(index, changed);
        }

        if !updated.is_empty() {
            self.bind_groups
                .update(updated, device, resources, &self.allocations);
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PassBindGroup {
    pub layout: u32,
    pub bind_group: u32,
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

        cache.entries = (0..allocations.len())
            .map(|index| allocations[index].generation)
            .collect();

        cache
    }

    pub fn get(&self, bind_group: u32) -> &BindGroup {
        &self.bind_groups[bind_group as usize].bind_group
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

    pub fn add(&mut self, archetype: u32, bind_group: CachedBindGroup) -> GpuResourceId<BindGroup> {
        let id = GpuResourceId::new(self.bind_groups.len() as u32);
        self.bind_groups.push(bind_group);
        self.archetypes[archetype as usize].bind_groups.push(id);

        id
    }

    pub fn update(
        &mut self,
        updated: FixedBitSet,
        device: &RenderDevice,
        resources: &GraphResources,
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
                    let node = &resources.nodes[allocation.node as usize];
                    let ty = &resources.types[node.ty as usize];
                    ty.bind(&allocation.instance, &mut builder);
                }

                cached.bind_group = builder.build(device, &cached.layout);
            }
        }
    }

    pub fn create_layouts(
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
