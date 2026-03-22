use crate::{
    core::{RenderDevice, RenderSettings},
    renderer::{
        camera::Camera,
        graph::{
            BoxData, ExecutableGraph, PassInstance, RenderGraph, ResourceAccess, ResourceResolver,
            allocator::{
                BindGroupCache, BindGroupKey, GpuAllocationDesc, GpuResourceAllocator,
                PassBindGroup,
            },
        },
    },
    resources::{BindGroupLayoutBuilder, BindGroupLayoutRegistry},
};
use ecs::{Entity, IndexSet, Query, World, query::With};
use std::collections::{HashMap, VecDeque};
use wgpu::BindGroupLayoutEntry;

pub struct RenderGraphCompiler;

impl RenderGraphCompiler {
    pub(crate) fn compile(
        world: &World,
        device: &RenderDevice,
        graph: &RenderGraph,
        settings: &RenderSettings,
        executable: &mut ExecutableGraph,
        layouts: &mut BindGroupLayoutRegistry,
        cameras: Query<Entity, With<Camera>>,
    ) {
        let CompiledRenderGraph {
            passes,
            resources,
            allocations,
            bind_group_layouts,
            bind_groups,
        } = Self::run(world, graph, settings, cameras);

        let layouts = BindGroupCache::create_layouts(device, layouts, bind_group_layouts);

        let allocator = GpuResourceAllocator::build(
            device,
            graph,
            resources,
            allocations,
            layouts,
            bind_groups,
        );

        executable.set(*settings, passes, allocator);
    }

    fn run(
        world: &World,
        graph: &RenderGraph,
        settings: &RenderSettings,
        cameras: Query<Entity, With<Camera>>,
    ) -> CompiledRenderGraph {
        let mut passes = Vec::new();
        let mut resources = vec![0u32; graph.resources.nodes.len()];
        let mut versions = Vec::new();
        let mut layouts = IndexSet::<Vec<BindGroupLayoutEntry>>::new();
        let mut bind_groups = IndexSet::<BindGroupKey>::new();

        // Expansion
        for camera in cameras {
            let offset = versions.len() as u32;
            let mut resolver = ResourceResolver::new(world, Some(camera));

            for pass in &graph.nodes {
                let pass_id = passes.len() as u32;
                let mut reads = Vec::new();

                for index in pass.entries() {
                    let version = versions.len() as u32;
                    let entry = graph.resources.entries[*index as usize];
                    let node = &graph.resources.nodes[entry.node as usize];
                    let ty = &graph.resources.types[node.ty as usize];

                    resolver.resource = node.id;

                    let desc = ty.resolve(world, settings, &mut resolver, ty.clone(&node.desc));
                    let (producer, user) = match entry.access {
                        ResourceAccess::Read => {
                            resources[node.id as usize] += 1;
                            reads.push(version);
                            (None, Some(pass_id))
                        }
                        ResourceAccess::Write => (Some(pass_id), Some(pass_id)),
                    };

                    versions.push(ResourceVersion {
                        id: version,
                        node: node.id,
                        producer,
                        user,
                        desc,
                    });
                }

                passes.push(PassRef {
                    node: pass.id,
                    resources: offset,
                    reads,
                    ref_count: pass.refs,
                });
            }
        }

        // Pass Culling
        let mut dead_versions = (0..versions.len())
            .filter(|i| resources[versions[*i].node as usize] == 0)
            .collect::<VecDeque<_>>();

        while let Some(version) = dead_versions.pop_front() {
            let Some(producer) = versions[version].producer else {
                continue;
            };

            let pass = &mut passes[producer as usize];
            if pass.ref_count == 0 {
                continue;
            }

            pass.ref_count -= 1;
            if pass.ref_count > 0 {
                continue;
            }

            let mut unused = Vec::new();
            for index in &pass.reads {
                let version = &versions[*index as usize];
                resources[version.node as usize] -= 1;
                if resources[version.node as usize] == 0 {
                    unused.push(version.node);
                }
            }

            dead_versions.extend(
                versions
                    .iter()
                    .filter(|v| unused.contains(&v.node))
                    .map(|v| v.id as usize),
            );
        }

        // Resource Allocation
        versions.retain(|v| resources[v.node as usize] > 0);
        versions.sort_by(|a, b| a.user.cmp(&b.user));
        let mut allocations = Vec::<GpuAllocationDesc>::new();
        let mut resources = vec![0u32; versions.len()];

        for version in versions {
            if let Some(index) = allocations.iter().position(|alloc| {
                let node = &graph.resources.nodes[alloc.node as usize];
                let version_node = &graph.resources.nodes[version.node as usize];
                let ty = &graph.resources.types[node.ty as usize];

                version.user.cmp(&alloc.last_use) == core::cmp::Ordering::Greater
                    && version_node.ty == node.ty
                    && version_node.kind == node.kind
                    && ty.compatible(&version.desc, &alloc.desc)
            }) {
                allocations[index].last_use = version.user;
                resources[version.id as usize] = index as u32;
            } else {
                let id = allocations.len() as u32;
                let node = &graph.resources.nodes[version.node as usize];
                resources[version.id as usize] = id;

                allocations.push(GpuAllocationDesc {
                    id,
                    node: node.id,
                    kind: node.kind,
                    last_use: version.user,
                    desc: version.desc,
                });
            }
        }

        // Pass Bindings
        passes.retain(|p| p.ref_count > 0);

        let instances = passes.iter().map(|pass| {
            let bindings = Self::get_bindings(
                graph,
                settings,
                pass,
                &resources,
                &allocations,
                &mut layouts,
                &mut bind_groups,
            );
            PassInstance {
                node: pass.node,
                resources: pass.resources,
                bindings: bindings.into_boxed_slice(),
            }
        });

        CompiledRenderGraph {
            passes: instances.collect(),
            resources: resources.into_boxed_slice(),
            allocations,
            bind_group_layouts: layouts,
            bind_groups,
        }
    }

    fn get_bindings(
        graph: &RenderGraph,
        settings: &RenderSettings,
        pass: &PassRef,
        resources: &[u32],
        allocations: &[GpuAllocationDesc],
        layouts: &mut IndexSet<Vec<BindGroupLayoutEntry>>,
        bind_groups: &mut IndexSet<BindGroupKey>,
    ) -> Vec<PassBindGroup> {
        let mut groups = HashMap::new();
        for binding in &graph.nodes[pass.node as usize].bindings {
            let entry = graph.resources.entries[binding.entry as usize];
            let node = &graph.resources.nodes[entry.node as usize];
            let ty = &graph.resources.types[node.ty as usize];
            let alloc = resources[(pass.resources + entry.id) as usize];
            let desc = &allocations[alloc as usize].desc;
            let group = groups
                .entry(binding.group)
                .or_insert_with(|| ResourceGroup::new(binding.group));

            group.allocations.push(alloc);
            ty.entry(settings, desc, &mut group.builder, binding.visiblitiy);
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

pub struct PassRef {
    node: u32,
    ref_count: u32,
    resources: u32,
    reads: Vec<u32>,
}

pub struct ResourceVersion {
    id: u32,
    node: u32,
    producer: Option<u32>,
    user: Option<u32>,
    desc: BoxData,
}

pub struct ResourceGroup {
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

pub struct CompiledRenderGraph {
    pub passes: Box<[PassInstance]>,
    pub resources: Box<[u32]>,
    pub allocations: Vec<GpuAllocationDesc>,
    pub bind_group_layouts: IndexSet<Vec<BindGroupLayoutEntry>>,
    pub bind_groups: IndexSet<BindGroupKey>,
}
