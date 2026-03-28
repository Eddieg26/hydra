use crate::{
    core::RenderSettings,
    renderer::{
        camera::CameraQueue,
        graph::{
            BoxData, PassId, PassInstance, RenderGraph, ResourceAccess, ResourceResolver,
            allocator::{BindGroupKey, GpuAllocationDesc, PassBindGroup},
        },
    },
    resources::BindGroupLayoutBuilder,
};
use ecs::{FixedBitSet, IndexDag, IndexSet, World, core::ImmutableIndexDag};
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
        let sorted = Self::sort(graph);

        let (passes, mut resources, mut ref_table) =
            Self::expand(world, graph, settings, cameras, sorted);

        let passes = Self::cull(graph, passes, &mut resources, &ref_table);

        let (allocations, alloc_table) = Self::allocate(graph, resources);

        let (passes, layouts, bind_groups) = Self::bindings(
            graph,
            settings,
            passes,
            alloc_table,
            &mut ref_table,
            &allocations,
        );

        CompiledRenderGraph {
            passes: passes.into_boxed_slice(),
            resources: ref_table.into_boxed_slice(),
            allocations,
            bind_group_layouts: layouts,
            bind_groups,
        }
    }

    fn sort(graph: &RenderGraph) -> ImmutableIndexDag<PassId> {
        let mut dag = IndexDag::default();
        let nodes = graph
            .nodes
            .iter()
            .map(|pass| dag.add_node(pass.id))
            .collect::<Vec<_>>();
        for node in nodes {
            for dependency in graph.nodes[node].dependencies().0.ones() {
                dag.add_dependency(dependency, node);
            }
        }

        dag.build_immutable().unwrap()
    }

    /// Expand render graph by instantiating camera passes.
    fn expand(
        world: &World,
        graph: &RenderGraph,
        settings: &RenderSettings,
        cameras: &CameraQueue,
        sorted: ImmutableIndexDag<PassId>,
    ) -> (Vec<PassRef>, Vec<ResourceRef>, Vec<u32>) {
        let mut passes = Vec::with_capacity(cameras.slice().len() * graph.nodes.len());
        let mut resources = Vec::<ResourceRef>::with_capacity(
            cameras.slice().len() * graph.resources().nodes().len(),
        );
        let mut table = vec![0; resources.capacity()];

        for index in 0..cameras.slice().len() {
            let camera = &cameras.slice()[index];
            let mut resolver = ResourceResolver::new(world, Some(camera));
            let cursor = resources.len() as u32;

            for pass in sorted
                .iter()
                .filter(|p| !camera.mask.as_ref().is_some_and(|mask| mask.get(**p)))
            {
                let pass = &graph.nodes[pass.0 as usize];
                let id = passes.len() as u32;
                let mut reads = Vec::new();
                let mut ref_count = 0;

                for entry in pass.entries() {
                    let resource = cursor + entry.node;
                    let node = graph.resources().node(entry.node);
                    let ty = graph.resources().ty(node.ty);
                    let desc = ty.resolve(world, settings, &mut resolver, ty.clone(&node.desc));

                    if let Some(index) = resources.iter().position(|r| {
                        let other_node = graph.resources().node(r.node);
                        let ty = graph.resources().ty(other_node.ty);

                        other_node.ty == node.ty && ty.compatible(&r.desc, &desc)
                    }) {
                        table[resource as usize] = index as u32;
                    } else {
                        table[resource as usize] = resources.len() as u32;
                        resources.push(ResourceRef {
                            id: resources.len() as u32,
                            node: node.id,
                            ref_count: if ty.output() { 1 } else { 0 },
                            first_user: None,
                            last_user: None,
                            writers: FixedBitSet::with_capacity(passes.len()),
                            desc,
                        });
                    }

                    match entry.access {
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
                    node: *pass.id,
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
        graph: &RenderGraph,
        mut passes: Vec<PassRef>,
        resources: &mut [ResourceRef],
        table: &[u32],
    ) -> Vec<PassRef> {
        let mut dead_resources = resources
            .iter()
            .filter_map(|r| (r.ref_count == 0).then_some(r.id))
            .collect::<VecDeque<_>>();

        while let Some(resource) = dead_resources.pop_front() {
            let mut dead_passes = Vec::new();
            for index in resources[resource as usize].writers.ones() {
                let pass = &mut passes[index as usize];

                if pass.ref_count == 0 {
                    continue;
                }

                pass.ref_count -= 1;
                if pass.ref_count == 0 {
                    dead_passes.push(index);
                }
            }

            for pass in dead_passes {
                for index in &passes[pass].reads {
                    let resource = &mut resources[table[*index as usize] as usize];
                    resource.ref_count -= 1;
                    if resource.ref_count == 0 {
                        dead_resources.push_back(resource.id);
                    }
                }
            }
        }

        passes
            .drain(..)
            .filter(|pass| {
                if pass.ref_count > 0 {
                    for entry in graph.nodes[pass.node as usize].entries() {
                        let resource = pass.cursor + entry.node;
                        resources[table[resource as usize] as usize].ref_count = 1;
                    }

                    true
                } else {
                    false
                }
            })
            .collect()
    }

    /// Allocate GPU resources.
    fn allocate(
        graph: &RenderGraph,
        mut resources: Vec<ResourceRef>,
    ) -> (Vec<GpuAllocationDesc>, Vec<u32>) {
        let mut allocations = Vec::<GpuAllocationDesc>::with_capacity(resources.len());
        let mut table = vec![0; resources.capacity()];

        resources.retain(|r| r.ref_count > 0);
        resources.sort_by(|a, b| a.first_user.cmp(&b.first_user));

        for resource in resources {
            let node = graph.resources().node(resource.node);
            let ty = graph.resources().ty(node.ty);

            if let Some(index) = allocations.iter().position(|alloc| {
                let other_node = graph.resources().node(alloc.node);
                alloc.last_user.cmp(&resource.first_user) == core::cmp::Ordering::Less
                    && other_node.ty == node.ty
                    && other_node.kind == node.kind
                    && ty.compatible(&alloc.desc, &resource.desc)
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
        alloc_table: Vec<u32>,
        ref_table: &mut [u32],
        allocations: &[GpuAllocationDesc],
    ) -> (
        Vec<PassInstance>,
        IndexSet<Vec<BindGroupLayoutEntry>>,
        IndexSet<BindGroupKey>,
    ) {
        let mut instances = Vec::with_capacity(passes.len());
        let mut layouts = IndexSet::default();
        let mut bind_groups = IndexSet::default();

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
                ref_table[(pass.cursor + binding.node) as usize] = alloc;
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
    first_user: Option<u32>,
    last_user: Option<u32>,
    writers: FixedBitSet,
    desc: BoxData,
}

impl ResourceRef {
    pub fn read(&mut self, pass: u32) {
        self.first_user = match self.first_user {
            Some(first) => Some(first.min(pass)),
            None => Some(pass),
        };

        self.last_user = match self.last_user {
            Some(last) => Some(last.max(pass)),
            None => Some(pass),
        };

        self.ref_count += 1;
    }

    pub fn write(&mut self, pass: u32) {
        self.first_user = match self.first_user {
            Some(first) => Some(first.min(pass)),
            None => Some(pass),
        };

        self.last_user = match self.last_user {
            Some(last) => Some(last.max(pass)),
            None => Some(pass),
        };

        self.writers.grow_and_insert(pass as usize);
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
            builder: BindGroupLayoutBuilder::default(),
            allocations: Vec::default(),
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

#[cfg(test)]
mod tests {
    use ecs::{Entity, World};
    use wgpu::ShaderStages;

    use crate::{
        core::{ColorFormat, RenderSettings},
        renderer::{
            camera::{CameraQueue, CameraSettings, SettingState},
            graph::{
                GraphPass, GraphResource, PassId, RenderGraph, RenderGraphMask, ResourceAccess,
                ResourceKind, ResourceUsage, compiler::RenderGraphCompiler,
            },
        },
        types::Viewport,
    };

    trait MockResource: GraphResource<Desc = u32> + Sized + Default {
        const OUTPUT: bool = false;

        fn kind() -> ResourceKind {
            ResourceKind::Transient
        }
    }

    impl<A: MockResource + Sized + Default> GraphResource for A {
        type Desc = u32;

        const OUTPUT: bool = <A as MockResource>::OUTPUT;

        fn resolve(
            _: &ecs::World,
            _: &crate::core::RenderSettings,
            _: &mut crate::renderer::graph::ResourceResolver,
            desc: Self::Desc,
        ) -> Self::Desc {
            desc
        }

        fn create(
            _: &ecs::World,
            _: &crate::core::RenderDevice,
            _: crate::renderer::graph::Name,
            _: &Self::Desc,
        ) -> Self {
            Self::default()
        }

        fn entry(
            _: &crate::core::RenderSettings,
            _: &Self::Desc,
            _: &mut crate::resources::BindGroupLayoutBuilder,
            _: wgpu::ShaderStages,
        ) {
        }

        fn bind<'a>(&'a self, _: &mut crate::resources::BindGroupBuilder<'a>) {}

        fn compatible(desc_a: &Self::Desc, desc_b: &Self::Desc) -> bool {
            desc_a >= desc_b
        }

        fn kind() -> ResourceKind {
            <A as MockResource>::kind()
        }
    }

    #[derive(Default)]
    struct MockResourceA;
    impl MockResource for MockResourceA {}

    #[derive(Default)]
    struct MockResourceB;
    impl MockResource for MockResourceB {}

    #[derive(Default)]
    struct MockOutputResource;
    impl MockResource for MockOutputResource {
        const OUTPUT: bool = true;

        fn kind() -> ResourceKind {
            ResourceKind::Imported
        }
    }

    struct PassA;
    impl GraphPass for PassA {
        const NAME: crate::renderer::graph::Name = "PassA";

        fn setup(
            builder: &mut crate::renderer::graph::PassBuilder,
        ) -> impl Fn(&mut crate::renderer::graph::RenderContext) + Send + Sync + 'static {
            builder.create::<MockResourceA>(
                "res_a",
                1024,
                ResourceUsage::Binding {
                    group: 0,
                    binding: 0,
                    visibility: ShaderStages::FRAGMENT,
                    access: ResourceAccess::Read,
                },
            );

            builder.create::<MockResourceB>("res_b", 1024, ResourceUsage::Attachment);

            |_| {}
        }
    }

    struct PassB;
    impl GraphPass for PassB {
        const NAME: crate::renderer::graph::Name = "PassB";

        fn setup(
            builder: &mut crate::renderer::graph::PassBuilder,
        ) -> impl Fn(&mut crate::renderer::graph::RenderContext) + Send + Sync + 'static {
            builder.create::<MockResourceB>(
                "res_b",
                1024,
                ResourceUsage::Binding {
                    group: 0,
                    binding: 0,
                    visibility: ShaderStages::FRAGMENT,
                    access: ResourceAccess::Read,
                },
            );

            builder.create::<MockResourceA>("res_a", 1024, ResourceUsage::Attachment);

            |_| {}
        }
    }

    struct PassC;
    impl GraphPass for PassC {
        const NAME: crate::renderer::graph::Name = "PassC";

        fn setup(
            builder: &mut crate::renderer::graph::PassBuilder,
        ) -> impl Fn(&mut crate::renderer::graph::RenderContext) + Send + Sync + 'static {
            builder.create::<MockResourceA>(
                "res_a",
                2048,
                ResourceUsage::Binding {
                    group: 0,
                    binding: 0,
                    visibility: ShaderStages::FRAGMENT,
                    access: ResourceAccess::Read,
                },
            );
            builder.create::<MockResourceB>("res_b", 1024, ResourceUsage::Attachment);

            |_| {}
        }
    }

    struct CulledPass;
    impl GraphPass for CulledPass {
        const NAME: crate::renderer::graph::Name = "Culled";

        fn setup(
            builder: &mut crate::renderer::graph::PassBuilder,
        ) -> impl Fn(&mut crate::renderer::graph::RenderContext) + Send + Sync + 'static {
            builder.create::<MockResourceA>(
                "res_b",
                2048,
                ResourceUsage::Binding {
                    group: 0,
                    binding: 0,
                    visibility: ShaderStages::FRAGMENT,
                    access: ResourceAccess::Read,
                },
            );

            |_| {}
        }
    }

    struct OutputPass;
    impl GraphPass for OutputPass {
        const NAME: crate::renderer::graph::Name = "OutputPass";

        fn setup(
            builder: &mut crate::renderer::graph::PassBuilder,
        ) -> impl Fn(&mut crate::renderer::graph::RenderContext) + Send + Sync + 'static {
            builder.create::<MockOutputResource>("output", 1024, ResourceUsage::Attachment);
            |_| {}
        }
    }

    struct WriteOutputPass;
    impl GraphPass for WriteOutputPass {
        const NAME: crate::renderer::graph::Name = "WriteOutputPass";

        fn setup(
            builder: &mut crate::renderer::graph::PassBuilder,
        ) -> impl Fn(&mut crate::renderer::graph::RenderContext) + Send + Sync + 'static {
            builder.create::<MockOutputResource>(
                "output",
                1024,
                ResourceUsage::Binding {
                    group: 0,
                    binding: 0,
                    visibility: ShaderStages::FRAGMENT,
                    access: ResourceAccess::Write,
                },
            );

            |_| {}
        }
    }

    fn create_camera(mask: Option<RenderGraphMask>) -> CameraSettings {
        CameraSettings {
            entity: Entity::new(0, 0),
            priority: 0,
            target: None,
            clear: None,
            viewport: Viewport::default(),
            msaa: SettingState::Auto,
            width: 1920,
            height: 1080,
            format: ColorFormat::Standard { srgb: false },
            mask,
            generation: 1,
        }
    }

    #[test]
    fn test_single_output_pass() {
        let mut graph = RenderGraph::default();
        graph.add_pass::<OutputPass>();

        let world = World::default();
        let settings = RenderSettings::default();
        let cameras = CameraQueue::from(vec![create_camera(None)]);

        let compiled = RenderGraphCompiler::run(&world, &graph, &settings, &cameras);
        assert_eq!(compiled.passes.len(), 1,);
    }

    #[test]
    fn test_sort() {
        let mut graph = RenderGraph::default();
        let pass_c = graph.add_pass::<PassC>();
        let pass_b = graph.add_before::<PassB>(pass_c);
        let pass_a = graph.add_before::<PassA>(pass_b);

        let world = World::default();
        let settings = RenderSettings::default();
        let cameras = CameraQueue::from(vec![create_camera(None)]);

        let compiled = RenderGraphCompiler::run(&world, &graph, &settings, &cameras);
        let nodes = compiled
            .passes
            .iter()
            .map(|pass| PassId(pass.node))
            .collect::<Vec<_>>();
        assert_eq!(nodes, vec![pass_a, pass_b, pass_c]);
    }

    #[test]
    fn test_expansion() {
        let mut graph = RenderGraph::default();
        let pass_a = graph.add_pass::<PassA>();
        graph.add_after::<PassB>(pass_a);

        let world = World::default();
        let settings = RenderSettings::default();
        let cameras = CameraQueue::from(vec![create_camera(None), create_camera(None)]);

        let compiled = RenderGraphCompiler::run(&world, &graph, &settings, &cameras);

        let pass_count = graph.node_map.len();
        let camera_count = cameras.slice().len();
        let result = pass_count * camera_count;
        assert_eq!(
            compiled.passes.len(),
            result,
            "{} passes x {} cameras should expand to {}",
            pass_count,
            camera_count,
            result
        );
    }

    #[test]
    fn test_cull() {
        let mut graph = RenderGraph::default();
        let pass_a = graph.add_pass::<PassA>();
        let pass_b = graph.add_after::<PassB>(pass_a);
        graph.add_after::<CulledPass>(pass_b);

        let world = World::default();
        let settings = RenderSettings::default();
        let cameras = CameraQueue::from(vec![create_camera(None)]);

        let compiled = RenderGraphCompiler::run(&world, &graph, &settings, &cameras);
        assert_eq!(compiled.passes.len(), 2);
    }

    #[test]
    fn test_disable_pass() {
        let mut graph = RenderGraph::default();
        graph.add_pass::<PassA>();
        graph.add_pass::<PassB>();
        let pass_c = graph.add_pass::<PassC>();

        let mut disabled = RenderGraphMask::new(graph.nodes.len());
        disabled.set(pass_c, true);

        let world = World::default();
        let settings = RenderSettings::default();
        let cameras = CameraQueue::from(vec![create_camera(Some(disabled))]);
        let compiled = RenderGraphCompiler::run(&world, &graph, &settings, &cameras);
        assert_eq!(compiled.passes.len(), 2);
    }

    #[test]
    fn test_single_allocation() {
        let mut graph = RenderGraph::default();
        let pass_id = graph.add_pass::<WriteOutputPass>();
        graph.add_after::<OutputPass>(pass_id);

        let world = World::default();
        let settings = RenderSettings::default();
        let cameras = CameraQueue::from(vec![create_camera(None)]);

        let compiled = RenderGraphCompiler::run(&world, &graph, &settings, &cameras);
        assert_eq!(compiled.allocations.len(), 1);
    }

    #[test]
    fn test_multi_allocation() {
        let mut graph = RenderGraph::default();
        graph.add_pass::<PassA>();
        graph.add_pass::<PassB>();
        graph.add_pass::<PassC>();

        let world = World::default();
        let settings = RenderSettings::default();
        let cameras = CameraQueue::from(vec![create_camera(None)]);

        let compiled = RenderGraphCompiler::run(&world, &graph, &settings, &cameras);
        assert_eq!(compiled.allocations.len(), 3);
    }

    #[test]
    fn test_shared_allocation() {
        let mut graph = RenderGraph::default();
        graph.add_pass::<PassC>();
        graph.add_pass::<PassA>();
        graph.add_pass::<PassB>();

        let world = World::default();
        let settings = RenderSettings::default();
        let cameras = CameraQueue::from(vec![create_camera(None)]);

        let compiled = RenderGraphCompiler::run(&world, &graph, &settings, &cameras);
        assert_eq!(compiled.allocations.len(), 2);
    }

    #[test]
    fn test_bindings() {
        let mut graph = RenderGraph::default();
        graph.add_pass::<PassA>();
        graph.add_pass::<PassB>();
        graph.add_pass::<PassC>();

        let world = World::default();
        let settings = RenderSettings::default();
        let cameras = CameraQueue::from(vec![create_camera(None)]);

        let compiled = RenderGraphCompiler::run(&world, &graph, &settings, &cameras);
        assert_eq!(compiled.bind_groups.len(), 3);
    }

    #[test]
    fn test_shared_bindings() {
        let mut graph = RenderGraph::default();
        graph.add_pass::<PassC>();
        graph.add_pass::<PassA>();
        graph.add_pass::<PassB>();

        let world = World::default();
        let settings = RenderSettings::default();
        let cameras = CameraQueue::from(vec![create_camera(None)]);

        let compiled = RenderGraphCompiler::run(&world, &graph, &settings, &cameras);
        assert_eq!(compiled.bind_groups.len(), 2);
    }
}
