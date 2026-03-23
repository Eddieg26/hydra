use crate::{
    core::RenderSettings,
    renderer::{
        camera::CameraQueue,
        graph::{
            BoxData, PassInstance, RenderGraph, ResourceAccess, ResourceResolver,
            allocator::{BindGroupKey, GpuAllocationDesc, PassBindGroup},
        },
    },
    resources::BindGroupLayoutBuilder,
};
use ecs::{IndexSet, World};
use std::collections::{HashMap, VecDeque};
use wgpu::BindGroupLayoutEntry;

pub struct RenderGraphCompiler;

impl RenderGraphCompiler {
    pub fn run(
        world: &World,
        graph: &RenderGraph,
        settings: &RenderSettings,
        cameras: &CameraQueue,
    ) -> CompiledRenderGraph {
        let (passes, mut resources, ref_table) = Self::expand(world, graph, settings, cameras);

        let passes = Self::cull(passes, &mut resources, &ref_table);

        let (allocations, alloc_table) = Self::allocate(graph, resources);

        let (passes, layouts, bind_groups) = Self::bindings(
            graph,
            settings,
            passes,
            ref_table,
            &alloc_table,
            &allocations,
        );

        CompiledRenderGraph {
            passes: passes.into_boxed_slice(),
            resources: alloc_table.into_boxed_slice(),
            allocations,
            bind_group_layouts: layouts,
            bind_groups,
        }
    }

    /// Expand render graph by instantiating camera passes.
    pub fn expand(
        world: &World,
        graph: &RenderGraph,
        settings: &RenderSettings,
        cameras: &CameraQueue,
    ) -> (Vec<PassRef>, Vec<ResourceRef>, Vec<u32>) {
        let mut passes = Vec::with_capacity(cameras.slice().len() * graph.nodes.len());
        let mut resources =
            Vec::<ResourceRef>::with_capacity(passes.len() * graph.resources().nodes().len());
        let mut table = vec![0; resources.len()];

        for index in 0..cameras.slice().len() {
            let camera = &cameras.slice()[index];
            let mut resolver = ResourceResolver::new(world, Some(camera));
            let cursor = resources.len() as u32;

            for pass in &graph.nodes {
                let id = passes.len() as u32;
                let mut reads = Vec::new();
                let mut ref_count = 0;

                for entry in pass.entries() {
                    let resource = cursor + entry.node;

                    match entry.access {
                        ResourceAccess::Create => {
                            let node = graph.resources().node(entry.node);
                            let ty = graph.resources().ty(node.ty);
                            let desc =
                                ty.resolve(world, settings, &mut resolver, ty.clone(&node.desc));

                            if let Some(index) = resources.iter().position(|r| {
                                let node = graph.resources().node(r.node);
                                let ty = graph.resources().ty(node.ty);

                                ty.compatible(&desc, &r.desc)
                            }) {
                                table[resource as usize] = index as u32;
                            } else {
                                table[resource as usize] = resources.len() as u32;
                                resources.push(ResourceRef {
                                    id: resources.len() as u32,
                                    node: node.id,
                                    ref_count: 0,
                                    producer: Some(id),
                                    first_user: None,
                                    last_user: None,
                                    desc,
                                });
                            }
                        }
                        ResourceAccess::Read => {
                            reads.push(resource);
                            resources[table[resource as usize] as usize].read(id);
                        }
                        ResourceAccess::Write => {
                            ref_count += 1;
                            resources[table[resource as usize] as usize].write(id);
                        }
                    }
                }

                passes.push(PassRef {
                    node: pass.id,
                    camera: Some(index as u32),
                    ref_count,
                    cursor,
                    reads,
                });
            }
        }

        (passes, resources, table)
    }

    /// Cull unused passes and resources.
    fn cull(
        mut passes: Vec<PassRef>,
        resources: &mut [ResourceRef],
        table: &[u32],
    ) -> Vec<PassRef> {
        let mut dead_resources = resources
            .iter()
            .filter_map(|r| (r.ref_count == 0).then_some(r.id))
            .collect::<VecDeque<_>>();

        while let Some(resource) = dead_resources.pop_front() {
            let Some(producer) = resources[resource as usize].producer else {
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

            for index in &pass.reads {
                let resource = &mut resources[table[*index as usize] as usize];
                resource.ref_count -= 1;
                if resource.ref_count == 0 {
                    dead_resources.push_back(*index);
                }
            }
        }

        passes.retain(|p| p.ref_count > 0);
        passes
    }

    /// Allocate GPU resources.
    fn allocate(
        graph: &RenderGraph,
        mut resources: Vec<ResourceRef>,
    ) -> (Vec<GpuAllocationDesc>, Vec<u32>) {
        let mut allocations = Vec::<GpuAllocationDesc>::with_capacity(resources.len());
        let mut table = vec![0; resources.len()];

        resources.retain(|r| r.ref_count > 0);
        resources.sort_by(|a, b| a.first_user.cmp(&b.first_user));

        for resource in resources {
            let node = graph.resources().node(resource.node);
            let ty = graph.resources().ty(node.ty);

            if let Some(index) = allocations.iter().position(|alloc| {
                let other_node = graph.resources().node(alloc.node);
                alloc.last_user.cmp(&resource.first_user) == core::cmp::Ordering::Greater
                    && other_node.ty == node.ty
                    && other_node.kind == node.kind
                    && ty.compatible(&resource.desc, &alloc.desc)
            }) {
                allocations[index].last_user = resource.last_user;
                table[resource.id as usize] = index as u32;
            } else {
                let id = allocations.len() as u32;

                table[resource.id as usize] = id;
                allocations.push(GpuAllocationDesc {
                    id: id as u32,
                    node: node.id,
                    kind: node.kind,
                    last_user: resource.last_user,
                    desc: resource.desc,
                });
            }
        }

        (allocations, table)
    }

    /// Build bind groups for each pass.
    fn bindings(
        graph: &RenderGraph,
        settings: &RenderSettings,
        passes: Vec<PassRef>,
        ref_table: Vec<u32>, // Node -> Resource Ref Index
        alloc_table: &[u32], // Ref -> Allocation index
        allocations: &[GpuAllocationDesc],
    ) -> (
        Vec<PassInstance>,
        IndexSet<Vec<BindGroupLayoutEntry>>,
        IndexSet<BindGroupKey>,
    ) {
        let mut instances = Vec::with_capacity(passes.len());
        let mut layouts = IndexSet::new();
        let mut bind_groups = IndexSet::new();

        for pass in passes {
            let mut groups = HashMap::new();
            for binding in &graph.nodes[pass.node as usize].bindings {
                let resource = ref_table[(pass.cursor + binding.node) as usize];
                let alloc = alloc_table[resource as usize];
                let node = &graph.resources.nodes[binding.node as usize];
                let ty = &graph.resources.types[node.ty as usize];
                let desc = &allocations[alloc as usize].desc;
                let group = groups
                    .entry(binding.group)
                    .or_insert_with(|| ResourceGroup::new(binding.group));

                group.allocations.push(alloc);
                ty.entry(settings, desc, &mut group.builder, binding.visiblitiy);
            }

            let mut groups = groups.into_values().collect::<Vec<_>>();
            groups.sort_by(|a, b| a.group.cmp(&b.group));

            let bindings = groups
                .drain(..)
                .map(|group| {
                    let (layout, _) = layouts.insert_full(group.builder.entries);
                    let allocations = group.allocations.into_boxed_slice();
                    let (bind_group, _) =
                        bind_groups.insert_full(BindGroupKey::new(layout as u32, allocations));
                    PassBindGroup {
                        layout: layout as u32,
                        bind_group: bind_group as u32,
                    }
                })
                .collect();

            instances.push(PassInstance {
                node: pass.node,
                cursor: pass.cursor,
                camera: pass.camera,
                bindings,
            });
        }

        (instances, layouts, bind_groups)
    }
}

pub struct PassRef {
    node: u32,
    camera: Option<u32>,
    ref_count: u32,
    cursor: u32,
    reads: Vec<u32>,
}

pub struct ResourceRef {
    id: u32,
    node: u32,
    ref_count: u32,
    producer: Option<u32>,
    first_user: Option<u32>,
    last_user: Option<u32>,
    desc: BoxData,
}

impl ResourceRef {
    pub fn read(&mut self, pass: u32) {
        self.first_user.get_or_insert(pass);
        self.last_user = Some(pass);
        self.ref_count += 1;
    }

    pub fn write(&mut self, pass: u32) {
        self.producer = Some(pass);
        self.first_user.get_or_insert(pass);
        self.last_user = Some(pass);
    }
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
