use crate::{
    renderer::graph::{
        PassAttachment, PassNode, PassRef, RenderGraph, ResourceBinding, ResourceKind, ResourceRef,
        ResourceVersion, SubGraphExpander,
        allocator::{BindGroupKey, GpuAllocationDesc},
    },
    resources::BindGroupLayoutBuilder,
};
use ecs::IndexSet;
use std::collections::HashMap;
use wgpu::BindGroupLayoutEntry;

pub struct RenderGraphCompiler;

impl RenderGraphCompiler {
    pub fn run(
        graph: &RenderGraph,
        layouts: &mut IndexSet<Vec<BindGroupLayoutEntry>>,
        bind_groups: &mut IndexSet<BindGroupKey>,
    ) -> GraphCompilation {
        let mut resources = graph
            .resources
            .nodes
            .iter()
            .map(ResourceRef::from)
            .collect::<Vec<_>>();
        let mut versions = Vec::new();
        let mut passes = Self::expand(graph, &mut resources, &mut versions);
        let active = Self::cull(&mut passes, &mut resources, &mut versions);

        let (table, allocations) = Self::allocate(graph, &resources, versions);

        let mut instances = Vec::<PassInstance>::with_capacity(passes.len());
        for index in active {
            let pass = &passes[index];
            let bindings = Self::bindings(graph, pass, &table, &allocations, layouts, bind_groups);
            instances.push(PassInstance {
                node: pass.node,
                bindings: bindings.into_boxed_slice(),
            });
        }

        GraphCompilation {
            passes: instances,
            resources: table,
            allocations,
        }
    }

    pub fn process(
        node: &PassNode,
        expander: &mut SubGraphExpander<'_>,
        versions: &mut Vec<u32>,
        bindings: &mut Vec<ResourceBinding>,
    ) {
        for attachment in &node.reads {
            let version = expander.add_resource(attachment.resource(), None);
            versions.push(version);

            if let PassAttachment::Binding {
                group,
                binding,
                stages,
                ..
            } = attachment
            {
                bindings.push(ResourceBinding {
                    resource: version,
                    group: *group,
                    binding: *binding,
                    stages: *stages,
                });
            }
        }
    }

    fn expand(
        graph: &RenderGraph,
        resources: &mut [ResourceRef],
        versions: &mut Vec<ResourceVersion>,
    ) -> Vec<PassRef> {
        let mut expander = SubGraphExpander::new(graph, resources, versions);
        let main_graph = &graph.passes.graphs[RenderGraph::MAIN_GRAPH];

        for pass in &main_graph.passes {
            let node = &expander.graph.passes.nodes[*pass as usize];
            let mut bindings: Vec<ResourceBinding> = Vec::new();
            let mut reads = Vec::with_capacity(node.reads.len());
            let mut writes = Vec::with_capacity(node.writes.len());

            Self::process(node, &mut expander, &mut reads, &mut bindings);
            Self::process(node, &mut expander, &mut writes, &mut bindings);

            expander.add_pass(*pass, reads, writes, bindings);
            node.expand(&mut expander);
        }

        expander.finish()
    }

    fn cull(
        passes: &mut [PassRef],
        resources: &mut [ResourceRef],
        versions: &mut [ResourceVersion],
    ) -> Vec<usize> {
        use std::collections::VecDeque;

        let mut unused_resources = versions
            .iter()
            .filter_map(|v| (resources[v.node as usize].ref_count > 0).then_some(v.id as usize))
            .collect::<VecDeque<_>>();

        while let Some(index) = unused_resources.pop_front() {
            let Some(user) = versions[index].producer else {
                continue;
            };

            let pass = &mut passes[user as usize];
            if pass.ref_count == 0 {
                continue;
            }

            pass.ref_count -= 1;
            if pass.ref_count > 0 {
                continue;
            }

            let mut unused = Vec::new();
            for index in &pass.reads {
                let input = &mut versions[*index as usize];
                let resource = &mut resources[input.node as usize];
                if resource.ref_count == 0 {
                    continue;
                }

                resource.ref_count -= 1;
                if resource.ref_count == 0 {
                    unused.push(index);
                }
            }

            unused_resources.extend(versions.iter().filter_map(|v| {
                (unused.contains(&&v.node) && pass.reads.contains(&v.id)).then_some(v.id as usize)
            }));
        }

        passes
            .iter()
            .enumerate()
            .filter_map(|(index, pass)| (pass.ref_count > 0).then_some(index))
            .collect()
    }

    fn allocate(
        graph: &RenderGraph,
        resources: &[ResourceRef],
        mut versions: Vec<ResourceVersion>,
    ) -> (Vec<u32>, Vec<GpuAllocationDesc>) {
        let mut table = vec![u32::MAX; versions.len()];
        let mut allocations = Vec::<GpuAllocationDesc>::with_capacity(versions.len());

        versions.retain(|v| resources[v.node as usize].ref_count > 0);
        versions.sort_by_key(|v| v.user);

        for version in versions {
            let node = &graph.resources.nodes[version.node as usize];
            if let Some(index) = Self::get_allocation(&version, node.kind, &allocations) {
                table[version.id as usize] = index as u32;
                allocations[index].last_use = version.user;
            } else {
                let index = allocations.len() as u32;
                table[version.id as usize] = index;
                allocations.push(GpuAllocationDesc {
                    id: index,
                    node: node.id,
                    key: version.key,
                    kind: node.kind,
                    last_use: version.user,
                    desc: version.desc,
                });
            }
        }

        (table, allocations)
    }

    fn get_allocation(
        version: &ResourceVersion,
        kind: ResourceKind,
        allocations: &[GpuAllocationDesc],
    ) -> Option<usize> {
        allocations.iter().position(|alloc| {
            alloc.compatible(version, kind)
                && version.user.cmp(&alloc.last_use) == core::cmp::Ordering::Greater
        })
    }

    fn bindings(
        graph: &RenderGraph,
        pass: &PassRef,
        table: &[u32],
        allocations: &[GpuAllocationDesc],
        layouts: &mut IndexSet<Vec<BindGroupLayoutEntry>>,
        bind_groups: &mut IndexSet<BindGroupKey>,
    ) -> Vec<PassBindGroup> {
        let mut groups = HashMap::new();
        for binding in &pass.bindings {
            let alloc = &allocations[table[binding.resource as usize] as usize];
            let node = &graph.resources.nodes[alloc.node as usize];
            let ty = &graph.resources.types[node.ty as usize];
            let group = groups
                .entry(binding.group)
                .or_insert_with(|| ResourceGroup::new(binding.group));

            group.allocations.push(alloc.id);
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

pub struct PassInstance {
    node: u32,
    bindings: Box<[PassBindGroup]>,
}

pub struct GraphCompilation {
    pub passes: Vec<PassInstance>,
    pub resources: Vec<u32>, // Table that points from version to allocation
    pub allocations: Vec<GpuAllocationDesc>,
}
