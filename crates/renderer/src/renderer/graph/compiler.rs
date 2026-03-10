use crate::{
    renderer::graph::{
        PassNode, RenderGraph, ResourceKind, ResourceNode, allocator::{BindGroupKey, GpuAllocationDesc},
    },
    resources::BindGroupLayoutBuilder,
};
use ecs::IndexSet;
use std::{cmp, collections::HashMap, hash::Hash};
use wgpu::BindGroupLayoutEntry;

pub struct GraphCompilation {
    pub passes: Vec<CompiledPass>,
    pub resources: Vec<u32>, // Table that points from version to allocation
    pub allocations: Vec<GpuAllocationDesc>,
}

#[derive(Default, Clone)]
struct ResourceRef {
    id: u32,
    ref_count: u32,
    producer: Option<u32>,
    first_use: Option<u32>,
    last_use: Option<u32>,
}

impl ResourceRef {
    pub fn reference(&mut self, pass: u32) {
        self.first_use = Some(match self.first_use {
            Some(old) => old.min(pass),
            None => pass,
        });
        self.last_use = Some(match self.last_use {
            Some(old) => old.max(pass),
            None => pass,
        });
    }
}

#[derive(Default, Clone)]
struct PassRef {
    id: u32,
    ref_count: u32,
}

struct ResourceGroup {
    group: u32,
    builder: BindGroupLayoutBuilder,
    allocations: Vec<u32>,
}

impl ResourceGroup {
    pub fn new(group: u32) -> Self {
        Self {
            group,
            builder: BindGroupLayoutBuilder::new(),
            allocations: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PassBindGroup {
    layout: u32,
    bind_group: u32,
}

pub struct CompiledPass {
    id: u32,
    bindings: Vec<PassBindGroup>,
}

pub struct RenderGraphCompiler<'a> {
    graph: &'a mut RenderGraph,
}

impl<'a> RenderGraphCompiler<'a> {
    pub fn run(
        &self,
        layouts: &mut IndexSet<Vec<BindGroupLayoutEntry>>,
        bind_groups: &mut IndexSet<BindGroupKey>,
    ) -> GraphCompilation {
        let mut resources = self.init_resources();
        let mut passes = self.init_passes(&mut resources);
        let mut culled = self.cull_passes(&mut passes, &mut resources);

        for index in &culled {
            let pass = &self.graph.passes[*index];

            for index in &pass.reads {
                resources[*index as usize].reference(pass.id);
            }

            for index in &pass.writes {
                resources[*index as usize].reference(pass.id);
            }
        }

        resources.retain(|r| r.ref_count > 0);
        resources.sort_by_key(|v| v.first_use);

        let (resources, allocations) = self.get_allocations(&resources);

        let passes = culled
            .drain(..)
            .map(|index| {
                let pass = &self.graph.passes[index];
                CompiledPass {
                    id: pass.id,
                    bindings: self.get_bindings(pass, &resources, layouts, bind_groups),
                }
            })
            .collect();

        GraphCompilation {
            passes,
            resources,
            allocations,
        }
    }

    fn init_resources(&self) -> Vec<ResourceRef> {
        let mut resources = vec![ResourceRef::default(); self.graph.resources.versions.len()];

        for resource in &self.graph.resources.versions {
            let resource_ref = &mut resources[resource.id as usize];
            resource_ref.id = resource.id;

            let node = &self.graph.resources.nodes[resource.node as usize];
            if node.root {
                resource_ref.ref_count = 1;
            }
        }

        resources
    }

    fn init_passes(&self, resources: &mut [ResourceRef]) -> Vec<PassRef> {
        let mut passes = vec![PassRef::default(); self.graph.passes.len()];

        for pass in &self.graph.passes {
            let pass_ref = &mut passes[pass.id as usize];
            pass_ref.id = pass.id;
            pass_ref.ref_count = (pass.writes.len() + pass.creates.len()) as u32;

            for index in &pass.creates {
                resources[*index as usize].producer = Some(pass.id);
            }

            for index in &pass.writes {
                resources[*index as usize].producer = Some(pass.id);
            }

            for index in &pass.reads {
                resources[*index as usize].ref_count += 1;
            }
        }

        passes
    }

    fn cull_passes(&self, passes: &mut [PassRef], resources: &mut [ResourceRef]) -> Vec<usize> {
        vec![]
    }

    fn get_bindings(
        &self,
        pass: &PassNode,
        table: &[u32],
        layouts: &mut IndexSet<Vec<BindGroupLayoutEntry>>,
        bind_groups: &mut IndexSet<BindGroupKey>,
    ) -> Vec<PassBindGroup> {
        let mut groups = HashMap::new();
        for binding in &pass.bindings {
            let node = &self.graph.resources.nodes[binding.resource as usize];
            let ty = &self.graph.resources.types[node.ty as usize];
            let group = groups
                .entry(binding.group)
                .or_insert_with(|| ResourceGroup::new(binding.group));

            group.allocations.push(table[node.id as usize]);
            ty.entry(&node.desc, &mut group.builder, binding.stages);
        }

        let mut groups = groups.into_values().collect::<Vec<_>>();
        groups.sort_by(|a, b| a.group.cmp(&b.group));

        groups
            .drain(..)
            .map(|group| {
                let (layout, _) = layouts.insert_full(group.builder.entries);
                let resources = group.allocations.into_boxed_slice();
                let (bind_group, _) =
                    bind_groups.insert_full(BindGroupKey::new(layout as u32, resources));
                PassBindGroup {
                    layout: layout as u32,
                    bind_group: bind_group as u32,
                }
            })
            .collect()
    }

    fn get_allocations(&self, resources: &[ResourceRef]) -> (Vec<u32>, Vec<GpuAllocationDesc>) {
        let mut table = vec![u32::MAX; self.graph.resources.versions.len()]; // Points resource version to allocation
        let mut allocations = Vec::<GpuAllocationDesc>::with_capacity(resources.len());

        for resource in resources {
            let node = self.graph.resources.versions[resource.id as usize].node;
            let node = &self.graph.resources.nodes[node as usize];
            if let Some(index) = Self::get_allocation(resource, node, &allocations) {
                table[resource.id as usize] = index as u32;
                allocations[index].last_use = resource.last_use;
            } else {
                let index = allocations.len() as u32;
                table[resource.id as usize] = index;
                allocations.push(GpuAllocationDesc {
                    id: index,
                    node: node.id,
                    desc_hash: node.desc_hash,
                    kind: node.kind,
                    last_use: resource.last_use,
                });
            }
        }

        (table, allocations)
    }

    fn get_allocation(
        resource: &ResourceRef,
        node: &ResourceNode,
        allocations: &[GpuAllocationDesc],
    ) -> Option<usize> {
        allocations.iter().position(|alloc| {
            alloc.compatible(node)
                && resource.first_use.cmp(&alloc.last_use) == cmp::Ordering::Greater
        })
    }
}
