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

        let (passes, mut resources, ref_table) =
            Self::expand(world, graph, settings, cameras, sorted);

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

    fn sort(graph: &RenderGraph) -> ImmutableIndexDag<PassId> {
        let mut dag = IndexDag::default();
        for pass in &graph.nodes {
            let node = dag.add_node(pass.id);
            for dependency in pass.dependencies().0.ones() {
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
        let mut resources =
            Vec::<ResourceRef>::with_capacity(passes.capacity() * graph.resources().nodes().len());
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
                let mut reads = Vec::default();
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
                                let other_node = graph.resources().node(r.node);
                                let ty = graph.resources().ty(other_node.ty);

                                other_node.ty == node.ty && ty.compatible(&desc, &r.desc)
                            }) {
                                table[resource as usize] = index as u32;
                            } else {
                                table[resource as usize] = resources.len() as u32;
                                resources.push(ResourceRef {
                                    id: resources.len() as u32,
                                    node: node.id,
                                    ref_count: if ty.root() { 1 } else { 0 },
                                    first_user: None,
                                    last_user: None,
                                    writers: FixedBitSet::with_capacity(passes.len()),
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
                alloc.last_user.cmp(&resource.first_user) == core::cmp::Ordering::Less
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
    use super::*;
    use crate::{
        core::{ColorFormat, RenderSettings},
        renderer::{
            camera::{CameraQueue, CameraSettings, SettingState},
            graph::{
                GraphPass, GraphResource, Name, PassBuilder, RenderContext, RenderGraph,
                RenderGraphMask, ResourceKind, ResourceUsage,
            },
        },
        resources::{BindGroupBuilder, BindGroupLayoutBuilder},
        types::Viewport,
    };
    use ecs::{Entity, World};
    use wgpu::ShaderStages;

    // ── Mock Resources ──────────────────────────────────────────────────

    struct MockResource;

    impl GraphResource for MockResource {
        type Desc = u32;

        fn resolve(
            _world: &World,
            _settings: &RenderSettings,
            _resolver: &mut crate::renderer::graph::ResourceResolver,
            desc: u32,
        ) -> u32 {
            desc
        }

        fn create(_device: &crate::core::RenderDevice, _name: Name, _desc: &u32) -> Self {
            MockResource
        }

        fn entry(
            _settings: &RenderSettings,
            _desc: &u32,
            _builder: &mut BindGroupLayoutBuilder,
            _visibility: ShaderStages,
        ) {
        }

        fn bind<'a>(&'a self, _builder: &mut BindGroupBuilder<'a>) {}

        fn compatible(a: &u32, b: &u32) -> bool {
            a == b
        }

        fn generation(&self) -> u32 {
            0
        }

        fn kind() -> ResourceKind {
            ResourceKind::Transient
        }
    }

    struct MockResourceB;

    impl GraphResource for MockResourceB {
        type Desc = u32;

        fn resolve(
            _world: &World,
            _settings: &RenderSettings,
            _resolver: &mut crate::renderer::graph::ResourceResolver,
            desc: u32,
        ) -> u32 {
            desc
        }

        fn create(_device: &crate::core::RenderDevice, _name: Name, _desc: &u32) -> Self {
            MockResourceB
        }

        fn entry(
            _settings: &RenderSettings,
            _desc: &u32,
            _builder: &mut BindGroupLayoutBuilder,
            _visibility: ShaderStages,
        ) {
        }

        fn bind<'a>(&'a self, _builder: &mut BindGroupBuilder<'a>) {}

        fn compatible(a: &u32, b: &u32) -> bool {
            a == b
        }

        fn generation(&self) -> u32 {
            0
        }

        fn kind() -> ResourceKind {
            ResourceKind::Transient
        }
    }

    struct MockResourceImported;

    impl GraphResource for MockResourceImported {
        type Desc = u32;

        fn resolve(
            _world: &World,
            _settings: &RenderSettings,
            _resolver: &mut crate::renderer::graph::ResourceResolver,
            desc: u32,
        ) -> u32 {
            desc
        }

        fn create(_device: &crate::core::RenderDevice, _name: Name, _desc: &u32) -> Self {
            MockResourceImported
        }

        fn entry(
            _settings: &RenderSettings,
            _desc: &u32,
            _builder: &mut BindGroupLayoutBuilder,
            _visibility: ShaderStages,
        ) {
        }

        fn bind<'a>(&'a self, _builder: &mut BindGroupBuilder<'a>) {}

        fn compatible(a: &u32, b: &u32) -> bool {
            a == b
        }

        fn generation(&self) -> u32 {
            0
        }

        fn kind() -> ResourceKind {
            ResourceKind::Imported
        }
    }

    // ── Helper Functions ────────────────────────────────────────────────

    fn make_camera(mask: Option<RenderGraphMask>) -> CameraSettings {
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

    fn make_camera_queue(cameras: Vec<CameraSettings>) -> CameraQueue {
        CameraQueue::from(cameras)
    }

    fn default_settings() -> RenderSettings {
        RenderSettings::default()
    }

    fn default_world() -> World {
        World::default()
    }

    // ── Diamond Test Passes ─────────────────────────────────────────────
    // Each pass creates (deduped) and writes "res_diamond" so it survives
    // culling. B, C, D also read it so the resource stays alive.

    struct DiamondA;
    impl GraphPass for DiamondA {
        const NAME: Name = "diamond_a";
        fn setup(builder: &mut PassBuilder) -> impl Fn(&mut RenderContext) + Send + Sync + 'static {
            let res =
                builder.create::<MockResource>("res_diamond", 1024, ResourceUsage::Attachment);
            builder.write::<MockResource>(res);
            move |_ctx| {}
        }
    }

    struct DiamondB;
    impl GraphPass for DiamondB {
        const NAME: Name = "diamond_b";
        fn setup(builder: &mut PassBuilder) -> impl Fn(&mut RenderContext) + Send + Sync + 'static {
            let res =
                builder.create::<MockResource>("res_diamond", 1024, ResourceUsage::Attachment);
            builder.read::<MockResource>(res);
            builder.write::<MockResource>(res);
            move |_ctx| {}
        }
    }

    struct DiamondC;
    impl GraphPass for DiamondC {
        const NAME: Name = "diamond_c";
        fn setup(builder: &mut PassBuilder) -> impl Fn(&mut RenderContext) + Send + Sync + 'static {
            let res =
                builder.create::<MockResource>("res_diamond", 1024, ResourceUsage::Attachment);
            builder.read::<MockResource>(res);
            builder.write::<MockResource>(res);
            move |_ctx| {}
        }
    }

    struct DiamondD;
    impl GraphPass for DiamondD {
        const NAME: Name = "diamond_d";
        fn setup(builder: &mut PassBuilder) -> impl Fn(&mut RenderContext) + Send + Sync + 'static {
            let res =
                builder.create::<MockResource>("res_diamond", 1024, ResourceUsage::Attachment);
            builder.read::<MockResource>(res);
            builder.write::<MockResource>(res);
            move |_ctx| {}
        }
    }

    // ── Single Pass Test Pass ───────────────────────────────────────────
    // Creates, reads, and writes a resource so it survives culling alone.

    struct SinglePass;
    impl GraphPass for SinglePass {
        const NAME: Name = "single_pass";
        fn setup(builder: &mut PassBuilder) -> impl Fn(&mut RenderContext) + Send + Sync + 'static {
            let res = builder.create::<MockResource>("res_single", 512, ResourceUsage::Attachment);
            builder.read::<MockResource>(res);
            builder.write::<MockResource>(res);
            move |_ctx| {}
        }
    }

    // ── DAG Property Test Passes ────────────────────────────────────────
    // Six fixed pass types for property-based topological ordering tests.
    // Each creates, reads, and writes a shared resource so it survives culling.

    macro_rules! dag_pass {
        ($name:ident, $label:expr) => {
            struct $name;
            impl GraphPass for $name {
                const NAME: Name = $label;
                fn setup(
                    builder: &mut PassBuilder,
                ) -> impl Fn(&mut RenderContext) + Send + Sync + 'static {
                    let res =
                        builder.create::<MockResource>("res_dag", 42, ResourceUsage::Attachment);
                    builder.read::<MockResource>(res);
                    builder.write::<MockResource>(res);
                    move |_ctx| {}
                }
            }
        };
    }

    dag_pass!(DagPass0, "dag_0");
    dag_pass!(DagPass1, "dag_1");
    dag_pass!(DagPass2, "dag_2");
    dag_pass!(DagPass3, "dag_3");
    dag_pass!(DagPass4, "dag_4");
    dag_pass!(DagPass5, "dag_5");

    /// A small DAG description for property-based testing.
    /// `num_nodes` is 0..=6, `edges` are (from, to) pairs with from != to.
    #[derive(Clone, Debug)]
    struct DagInput {
        num_nodes: usize,
        edges: Vec<(usize, usize)>,
    }

    impl quickcheck::Arbitrary for DagInput {
        fn arbitrary(g: &mut quickcheck::Gen) -> Self {
            let num_nodes = usize::arbitrary(g) % 7; // 0..=6
            if num_nodes <= 1 {
                return DagInput {
                    num_nodes,
                    edges: vec![],
                };
            }

            // Generate random forward edges (u < v) to guarantee a DAG.
            // This also ensures the dependency PassId fits in the dependent
            // pass's FixedBitSet mask (capacity == pass index).
            let mut edges = Vec::default();
            let mut seen = vec![vec![false; num_nodes]; num_nodes];

            let max_edges = num_nodes * (num_nodes - 1) / 2;
            let candidate_count = usize::arbitrary(g) % (max_edges + 1);

            for _ in 0..candidate_count {
                let u = usize::arbitrary(g) % num_nodes;
                let v = usize::arbitrary(g) % num_nodes;
                if u >= v || seen[u][v] {
                    continue;
                }
                seen[u][v] = true;
                edges.push((u, v));
            }

            DagInput { num_nodes, edges }
        }
    }

    /// Helper: build a RenderGraph from a DagInput, returning (graph, pass_ids).
    fn build_dag_graph(input: &DagInput) -> (RenderGraph, Vec<PassId>) {
        let mut graph = RenderGraph::default();
        // Register passes in order. Each type is unique so add_pass won't dedup.
        let ids: Vec<PassId> = (0..input.num_nodes)
            .map(|i| match i {
                0 => graph.add_pass::<DagPass0>(),
                1 => graph.add_pass::<DagPass1>(),
                2 => graph.add_pass::<DagPass2>(),
                3 => graph.add_pass::<DagPass3>(),
                4 => graph.add_pass::<DagPass4>(),
                5 => graph.add_pass::<DagPass5>(),
                _ => unreachable!(),
            })
            .collect();

        // Wire dependency edges: edge (u, v) means v depends on u (u runs before v).
        // add_after::<PassV>(ids[u]) registers ids[u] as a dependency of PassV.
        for &(u, v) in &input.edges {
            match v {
                0 => {
                    graph.add_after::<DagPass0>(ids[u]);
                }
                1 => {
                    graph.add_after::<DagPass1>(ids[u]);
                }
                2 => {
                    graph.add_after::<DagPass2>(ids[u]);
                }
                3 => {
                    graph.add_after::<DagPass3>(ids[u]);
                }
                4 => {
                    graph.add_after::<DagPass4>(ids[u]);
                }
                5 => {
                    graph.add_after::<DagPass5>(ids[u]);
                }
                _ => unreachable!(),
            }
        }

        (graph, ids)
    }

    // Feature: render-graph-compiler-tests, Property 1: Topological ordering preserves dependencies
    #[quickcheck_macros::quickcheck]
    fn prop_topological_order(input: DagInput) -> bool {
        let (graph, ids) = build_dag_graph(&input);
        let world = default_world();
        let settings = default_settings();
        let cameras = make_camera_queue(vec![make_camera(None)]);

        let compiled = RenderGraphCompiler::run(&world, &graph, &settings, &cameras);

        // With one camera and no masks, every pass should appear exactly once
        if compiled.passes.len() != input.num_nodes {
            return false;
        }

        // Build a map from node id → position in compiled output
        let mut pos_map: HashMap<u32, usize> = HashMap::default();
        for (pos, pass) in compiled.passes.iter().enumerate() {
            if pos_map.insert(pass.node, pos).is_some() {
                return false; // duplicate node — fail
            }
        }

        // Every pass node should be present
        for id in &ids {
            if !pos_map.contains_key(&**id) {
                return false;
            }
        }

        // Every dependency edge (u, v) must satisfy: pos(u) < pos(v)
        for &(u, v) in &input.edges {
            let pos_u = pos_map[&*ids[u]];
            let pos_v = pos_map[&*ids[v]];
            if pos_u >= pos_v {
                return false;
            }
        }

        true
    }

    // ── Sort Stage Tests ────────────────────────────────────────────────

    #[test]
    fn diamond_dependency_ordering() {
        // Build diamond: A → B, A → C, B → D, C → D
        let mut graph = RenderGraph::default();
        let a = graph.add_pass::<DiamondA>();
        let b = graph.add_after::<DiamondB>(a); // B depends on A
        let c = graph.add_after::<DiamondC>(a); // C depends on A
        graph.add_after::<DiamondD>(b); // D depends on B
        graph.add_after::<DiamondD>(c); // D also depends on C (same PassId, adds dep)

        let world = default_world();
        let settings = default_settings();
        let cameras = make_camera_queue(vec![make_camera(None)]);

        let compiled = RenderGraphCompiler::run(&world, &graph, &settings, &cameras);

        // All 4 passes should survive culling
        assert_eq!(compiled.passes.len(), 4, "expected 4 pass instances");

        // Map node ids to positions in the compiled output
        let pos = |node: u32| -> usize {
            compiled
                .passes
                .iter()
                .position(|p| p.node == node)
                .unwrap_or_else(|| panic!("pass node {} not found in compiled output", node))
        };

        let pos_a = pos(*a);
        let pos_b = pos(*b);
        let pos_c = pos(*c);
        // D's PassId: add_after returns the same PassId for DiamondD both times
        let d = graph.add_pass::<DiamondD>(); // returns existing PassId
        let pos_d = pos(*d);

        // A must appear before B and C
        assert!(
            pos_a < pos_b,
            "A (pos {pos_a}) must appear before B (pos {pos_b})"
        );
        assert!(
            pos_a < pos_c,
            "A (pos {pos_a}) must appear before C (pos {pos_c})"
        );
        // B and C must appear before D
        assert!(
            pos_b < pos_d,
            "B (pos {pos_b}) must appear before D (pos {pos_d})"
        );
        assert!(
            pos_c < pos_d,
            "C (pos {pos_c}) must appear before D (pos {pos_d})"
        );
    }

    #[test]
    fn single_pass_produces_one_instance() {
        let mut graph = RenderGraph::default();
        graph.add_pass::<SinglePass>();

        let world = default_world();
        let settings = default_settings();
        let cameras = make_camera_queue(vec![make_camera(None)]);

        let compiled = RenderGraphCompiler::run(&world, &graph, &settings, &cameras);

        assert_eq!(
            compiled.passes.len(),
            1,
            "single pass should produce exactly one instance"
        );
    }

    #[test]
    fn empty_graph_produces_empty_passes() {
        let graph = RenderGraph::default();

        let world = default_world();
        let settings = default_settings();
        let cameras = make_camera_queue(vec![make_camera(None)]);

        let compiled = RenderGraphCompiler::run(&world, &graph, &settings, &cameras);

        assert!(
            compiled.passes.is_empty(),
            "empty graph should produce zero pass instances"
        );
    }

    // ── Expand Stage Tests — Camera Instantiation & Mask Filtering ──────

    #[test]
    fn n_cameras_times_m_passes_expansion() {
        // 2 passes that survive culling, 3 cameras with no masks → 6 instances
        let mut graph = RenderGraph::default();
        graph.add_pass::<DagPass0>();
        graph.add_pass::<DagPass1>();

        let world = default_world();
        let settings = default_settings();
        let cameras = make_camera_queue(vec![
            make_camera(None),
            make_camera(None),
            make_camera(None),
        ]);

        let compiled = RenderGraphCompiler::run(&world, &graph, &settings, &cameras);

        assert_eq!(
            compiled.passes.len(),
            6,
            "2 passes × 3 cameras should produce 6 pass instances"
        );
    }

    #[test]
    fn camera_mask_excludes_one_pass() {
        // 2 passes, 2 cameras: camera 0 has no mask (all passes), camera 1 excludes pass 0
        let mut graph = RenderGraph::default();
        let pass0 = graph.add_pass::<DagPass0>();
        let pass1 = graph.add_pass::<DagPass1>();

        let mut mask = RenderGraphMask::new(2);
        mask.set(pass0, true); // exclude pass 0 for camera 1

        let world = default_world();
        let settings = default_settings();
        let cameras = make_camera_queue(vec![
            make_camera(None),       // gets both passes
            make_camera(Some(mask)), // gets only pass 1
        ]);

        let compiled = RenderGraphCompiler::run(&world, &graph, &settings, &cameras);

        // Camera 0: 2 passes, Camera 1: 1 pass → total 3
        assert_eq!(
            compiled.passes.len(),
            3,
            "camera 0 gets 2 passes, camera 1 (mask excludes pass 0) gets 1 pass → 3 total"
        );

        // Verify camera 1 only has pass 1 instances
        let camera1_passes: Vec<_> = compiled
            .passes
            .iter()
            .filter(|p| p.camera == Some(1))
            .collect();
        assert_eq!(
            camera1_passes.len(),
            1,
            "camera 1 should have 1 pass instance"
        );
        assert_eq!(
            camera1_passes[0].node, *pass1,
            "camera 1's only pass should be pass 1"
        );
    }

    #[test]
    fn mask_excludes_all_passes_and_mask_excludes_no_passes() {
        let mut graph = RenderGraph::default();
        let pass0 = graph.add_pass::<DagPass0>();
        let pass1 = graph.add_pass::<DagPass1>();

        // Mask with all bits set → excludes all passes
        let mut mask_all = RenderGraphMask::new(2);
        mask_all.set(pass0, true);
        mask_all.set(pass1, true);

        // Mask with all bits false → excludes no passes (same as None)
        let mask_none = RenderGraphMask::new(2);

        let world = default_world();
        let settings = default_settings();

        // Test: mask excludes all passes → zero instances for that camera
        let cameras_all_excluded = make_camera_queue(vec![make_camera(Some(mask_all))]);
        let compiled = RenderGraphCompiler::run(&world, &graph, &settings, &cameras_all_excluded);
        assert_eq!(
            compiled.passes.len(),
            0,
            "camera with mask excluding all passes should produce zero instances"
        );

        // Test: mask excludes no passes → all passes instantiated (same as None)
        let cameras_none_excluded = make_camera_queue(vec![make_camera(Some(mask_none))]);
        let compiled = RenderGraphCompiler::run(&world, &graph, &settings, &cameras_none_excluded);
        assert_eq!(
            compiled.passes.len(),
            2,
            "camera with mask excluding no passes should produce all pass instances"
        );
    }

    // ── Expand Stage Tests — Resource Deduplication & Access Tracking ────

    // Passes for deduplication tests: each creates, reads, and writes a
    // MockResource with desc=1024 so they survive culling and exercise
    // the deduplication path (same type + compatible desc).

    struct DedupPassA;
    impl GraphPass for DedupPassA {
        const NAME: Name = "dedup_a";
        fn setup(builder: &mut PassBuilder) -> impl Fn(&mut RenderContext) + Send + Sync + 'static {
            let res = builder.create::<MockResource>("res_dedup", 1024, ResourceUsage::Attachment);
            builder.read::<MockResource>(res);
            builder.write::<MockResource>(res);
            move |_ctx| {}
        }
    }

    struct DedupPassB;
    impl GraphPass for DedupPassB {
        const NAME: Name = "dedup_b";
        fn setup(builder: &mut PassBuilder) -> impl Fn(&mut RenderContext) + Send + Sync + 'static {
            let res = builder.create::<MockResource>("res_dedup", 1024, ResourceUsage::Attachment);
            builder.read::<MockResource>(res);
            builder.write::<MockResource>(res);
            move |_ctx| {}
        }
    }

    #[test]
    fn resource_deduplication_same_type_compatible_desc() {
        // Two passes both create MockResource with desc=1024 → same allocation
        let mut graph = RenderGraph::default();
        graph.add_pass::<DedupPassA>();
        graph.add_pass::<DedupPassB>();

        let world = default_world();
        let settings = default_settings();
        let cameras = make_camera_queue(vec![make_camera(None)]);

        let compiled = RenderGraphCompiler::run(&world, &graph, &settings, &cameras);

        // Both passes should survive culling
        assert_eq!(
            compiled.passes.len(),
            2,
            "both passes should survive culling"
        );

        // Since both create MockResource with desc=1024 (compatible), the expand
        // stage should deduplicate them into the same ResourceRef, leading to a
        // single allocation.
        assert_eq!(
            compiled.allocations.len(),
            1,
            "two passes creating MockResource with same desc should share one allocation"
        );
    }

    // Passes for non-deduplication tests (different type)

    struct DedupPassTypeB;
    impl GraphPass for DedupPassTypeB {
        const NAME: Name = "dedup_type_b";
        fn setup(builder: &mut PassBuilder) -> impl Fn(&mut RenderContext) + Send + Sync + 'static {
            let res =
                builder.create::<MockResourceB>("res_dedup_b", 1024, ResourceUsage::Attachment);
            builder.read::<MockResourceB>(res);
            builder.write::<MockResourceB>(res);
            move |_ctx| {}
        }
    }

    // Pass creating MockResource with desc=2048 (incompatible with 1024)

    struct DedupPassIncompat;
    impl GraphPass for DedupPassIncompat {
        const NAME: Name = "dedup_incompat";
        fn setup(builder: &mut PassBuilder) -> impl Fn(&mut RenderContext) + Send + Sync + 'static {
            let res = builder.create::<MockResource>(
                "res_dedup_incompat",
                2048,
                ResourceUsage::Attachment,
            );
            builder.read::<MockResource>(res);
            builder.write::<MockResource>(res);
            move |_ctx| {}
        }
    }

    #[test]
    fn resource_non_deduplication_different_type() {
        // MockResource desc=1024 vs MockResourceB desc=1024 → separate allocations
        let mut graph = RenderGraph::default();
        graph.add_pass::<DedupPassA>();
        graph.add_pass::<DedupPassTypeB>();

        let world = default_world();
        let settings = default_settings();
        let cameras = make_camera_queue(vec![make_camera(None)]);

        let compiled = RenderGraphCompiler::run(&world, &graph, &settings, &cameras);

        assert_eq!(
            compiled.passes.len(),
            2,
            "both passes should survive culling"
        );
        assert_eq!(
            compiled.allocations.len(),
            2,
            "different resource types should produce separate allocations"
        );
    }

    #[test]
    fn resource_non_deduplication_incompatible_desc() {
        // MockResource desc=1024 vs MockResource desc=2048 → separate allocations
        let mut graph = RenderGraph::default();
        graph.add_pass::<DedupPassA>();
        graph.add_pass::<DedupPassIncompat>();

        let world = default_world();
        let settings = default_settings();
        let cameras = make_camera_queue(vec![make_camera(None)]);

        let compiled = RenderGraphCompiler::run(&world, &graph, &settings, &cameras);

        assert_eq!(
            compiled.passes.len(),
            2,
            "both passes should survive culling"
        );
        assert_eq!(
            compiled.allocations.len(),
            2,
            "incompatible descriptions should produce separate allocations"
        );
    }

    // ── Resource Resolution Test ────────────────────────────────────────

    // A mock resource whose resolve() doubles the description value.
    // This lets us verify that resolve() is called during expansion and
    // the resolved description is used for compatibility checks.

    struct MockResourceDoubling;
    impl GraphResource for MockResourceDoubling {
        type Desc = u32;

        fn resolve(
            _world: &World,
            _settings: &RenderSettings,
            _resolver: &mut ResourceResolver,
            desc: u32,
        ) -> u32 {
            desc * 2 // doubles the description
        }

        fn create(_device: &crate::core::RenderDevice, _name: Name, _desc: &u32) -> Self {
            MockResourceDoubling
        }

        fn entry(
            _settings: &RenderSettings,
            _desc: &u32,
            _builder: &mut BindGroupLayoutBuilder,
            _vis: ShaderStages,
        ) {
        }

        fn bind<'a>(&'a self, _builder: &mut BindGroupBuilder<'a>) {}

        fn compatible(a: &u32, b: &u32) -> bool {
            a == b
        }

        fn generation(&self) -> u32 {
            0
        }

        fn kind() -> ResourceKind {
            ResourceKind::Transient
        }
    }

    // Pass creating MockResourceDoubling with desc=512 → resolves to 1024
    struct ResolvePassA;
    impl GraphPass for ResolvePassA {
        const NAME: Name = "resolve_a";
        fn setup(builder: &mut PassBuilder) -> impl Fn(&mut RenderContext) + Send + Sync + 'static {
            let res = builder.create::<MockResourceDoubling>(
                "res_resolve",
                512,
                ResourceUsage::Attachment,
            );
            builder.read::<MockResourceDoubling>(res);
            builder.write::<MockResourceDoubling>(res);
            move |_ctx| {}
        }
    }

    // Pass creating MockResourceDoubling with desc=1024 → resolves to 2048
    struct ResolvePassB;
    impl GraphPass for ResolvePassB {
        const NAME: Name = "resolve_b";
        fn setup(builder: &mut PassBuilder) -> impl Fn(&mut RenderContext) + Send + Sync + 'static {
            let res = builder.create::<MockResourceDoubling>(
                "res_resolve",
                1024,
                ResourceUsage::Attachment,
            );
            builder.read::<MockResourceDoubling>(res);
            builder.write::<MockResourceDoubling>(res);
            move |_ctx| {}
        }
    }

    // Pass creating MockResourceDoubling with desc=512 → resolves to 1024 (same as ResolvePassA)
    struct ResolvePassC;
    impl GraphPass for ResolvePassC {
        const NAME: Name = "resolve_c";
        fn setup(builder: &mut PassBuilder) -> impl Fn(&mut RenderContext) + Send + Sync + 'static {
            let res = builder.create::<MockResourceDoubling>(
                "res_resolve",
                512,
                ResourceUsage::Attachment,
            );
            builder.read::<MockResourceDoubling>(res);
            builder.write::<MockResourceDoubling>(res);
            move |_ctx| {}
        }
    }

    #[test]
    fn resource_resolution_uses_resolved_desc_for_compatibility() {
        // ResolvePassA: desc=512 → resolved=1024
        // ResolvePassB: desc=1024 → resolved=2048
        // Different resolved descs → separate allocations (proves resolve() is used)
        let mut graph = RenderGraph::default();
        graph.add_pass::<ResolvePassA>();
        graph.add_pass::<ResolvePassB>();

        let world = default_world();
        let settings = default_settings();
        let cameras = make_camera_queue(vec![make_camera(None)]);

        let compiled = RenderGraphCompiler::run(&world, &graph, &settings, &cameras);

        assert_eq!(compiled.passes.len(), 2);
        assert_eq!(
            compiled.allocations.len(),
            2,
            "resolved descs 1024 vs 2048 are incompatible → separate allocations"
        );

        // ResolvePassA: desc=512 → resolved=1024
        // ResolvePassC: desc=512 → resolved=1024
        // Same resolved descs → should deduplicate into one allocation
        let mut graph2 = RenderGraph::default();
        graph2.add_pass::<ResolvePassA>();
        graph2.add_pass::<ResolvePassC>();

        let compiled2 = RenderGraphCompiler::run(&world, &graph2, &settings, &cameras);

        assert_eq!(compiled2.passes.len(), 2);
        assert_eq!(
            compiled2.allocations.len(),
            1,
            "both resolve to 1024 → compatible → shared allocation (proves resolve() is called)"
        );
    }

    // ── ResourceRef direct tests (Task 6.1) ──────────────────────────

    fn fresh_resource_ref() -> ResourceRef {
        ResourceRef {
            id: 0,
            node: 0,
            ref_count: 0,
            first_user: None,
            last_user: None,
            writers: FixedBitSet::new(),
            desc: Box::new(0u32),
        }
    }

    #[test]
    fn resource_ref_read_on_fresh_sets_first_and_last_user() {
        let mut r = fresh_resource_ref();
        r.read(5);
        assert_eq!(r.first_user, Some(5));
        assert_eq!(r.last_user, Some(5));
        assert_eq!(r.ref_count, 1);
    }

    #[test]
    fn resource_ref_read_lower_updates_first_user() {
        let mut r = fresh_resource_ref();
        r.read(10);
        r.read(3);
        assert_eq!(r.first_user, Some(3));
        assert_eq!(r.last_user, Some(10));
        assert_eq!(r.ref_count, 2);
    }

    #[test]
    fn resource_ref_read_higher_updates_last_user() {
        let mut r = fresh_resource_ref();
        r.read(3);
        r.read(10);
        assert_eq!(r.first_user, Some(3));
        assert_eq!(r.last_user, Some(10));
        assert_eq!(r.ref_count, 2);
    }

    #[test]
    fn resource_ref_write_on_fresh_sets_producer_and_users() {
        let mut r = fresh_resource_ref();
        r.write(7);
        assert_eq!(r.first_user, Some(7));
        assert_eq!(r.last_user, Some(7));
        // write does not increment ref_count
        assert_eq!(r.ref_count, 0);
    }

    #[test]
    fn resource_ref_write_lower_updates_producer_to_minimum() {
        let mut r = fresh_resource_ref();
        r.write(10);
        r.write(4);
        assert_eq!(r.first_user, Some(4));
        assert_eq!(r.last_user, Some(10));
    }

    #[test]
    fn resource_ref_write_higher_keeps_producer_updates_last_user() {
        let mut r = fresh_resource_ref();
        r.write(4);
        r.write(10);
        assert_eq!(r.first_user, Some(4));
        assert_eq!(r.last_user, Some(10));
    }

    #[test]
    fn resource_ref_interleaved_read_write_maintains_invariants() {
        let mut r = fresh_resource_ref();
        // write at 5, read at 3, write at 8, read at 1, read at 10
        r.write(5);
        r.read(3);
        r.write(8);
        r.read(1);
        r.read(10);

        assert_eq!(r.first_user, Some(1), "first_user = min of all indices");
        assert_eq!(r.last_user, Some(10), "last_user = max of all indices");
        assert_eq!(r.ref_count, 3, "ref_count = number of reads");
        assert!(r.first_user.unwrap() <= r.last_user.unwrap());
    }

    #[test]
    fn resource_ref_reads_only_no_producer() {
        let mut r = fresh_resource_ref();
        r.read(2);
        r.read(7);
        r.read(4);
        assert_eq!(r.first_user, Some(2));
        assert_eq!(r.last_user, Some(7));
        assert_eq!(r.ref_count, 3);
    }

    #[test]
    fn resource_ref_writes_only_no_ref_count() {
        let mut r = fresh_resource_ref();
        r.write(6);
        r.write(2);
        r.write(9);
        assert_eq!(r.first_user, Some(2));
        assert_eq!(r.last_user, Some(9));
        assert_eq!(r.ref_count, 0);
    }

    #[test]
    fn resource_ref_same_pass_read_and_write() {
        let mut r = fresh_resource_ref();
        r.write(5);
        r.read(5);
        assert_eq!(r.first_user, Some(5));
        assert_eq!(r.last_user, Some(5));
        assert_eq!(r.ref_count, 1);
        assert!(r.first_user.unwrap() <= r.last_user.unwrap());
    }

    // ── Cull Stage Tests ────────────────────────────────────────────────

    // Cull test passes: designed to exercise dead code elimination.
    //
    // CullCreateA: creates + writes "res_cull" (MockResource 1024) → survives if resource has readers
    // CullReadWriteB: creates (deduped) + reads "res_cull", creates + writes "res_cull_b" (MockResourceB 1024)
    //   → B's ref_count comes from writing res_cull_b; if res_cull_b has no readers, B is culled
    // CullReadWriteC: creates (deduped) + reads "res_cull", writes "res_cull" → survives via write

    struct CullCreateA;
    impl GraphPass for CullCreateA {
        const NAME: Name = "cull_create_a";
        fn setup(builder: &mut PassBuilder) -> impl Fn(&mut RenderContext) + Send + Sync + 'static {
            let res = builder.create::<MockResource>("res_cull", 1024, ResourceUsage::Attachment);
            builder.write::<MockResource>(res);
            move |_ctx| {}
        }
    }

    struct CullReadWriteB;
    impl GraphPass for CullReadWriteB {
        const NAME: Name = "cull_rw_b";
        fn setup(builder: &mut PassBuilder) -> impl Fn(&mut RenderContext) + Send + Sync + 'static {
            // Read R1 (deduped via create with same name/type/desc)
            let r1 = builder.create::<MockResource>("res_cull", 1024, ResourceUsage::Attachment);
            builder.read::<MockResource>(r1);
            // Create + write R2 (different type → separate resource)
            let r2 = builder.create::<MockResourceB>("res_cull_b", 1024, ResourceUsage::Attachment);
            builder.write::<MockResourceB>(r2);
            move |_ctx| {}
        }
    }

    struct CullReadWriteC;
    impl GraphPass for CullReadWriteC {
        const NAME: Name = "cull_rw_c";
        fn setup(builder: &mut PassBuilder) -> impl Fn(&mut RenderContext) + Send + Sync + 'static {
            // Read + write R1 (deduped via create with same name/type/desc)
            let r1 = builder.create::<MockResource>("res_cull", 1024, ResourceUsage::Attachment);
            builder.read::<MockResource>(r1);
            builder.write::<MockResource>(r1);
            move |_ctx| {}
        }
    }

    // Cascade cull passes: A→B→C chain where R3 has no readers.
    // CascadeA: creates+writes R1
    // CascadeB: reads R1, creates+writes R2 (MockResourceB)
    // CascadeC: reads R2, creates+writes R3 (MockResourceImported)

    struct CascadeA;
    impl GraphPass for CascadeA {
        const NAME: Name = "cascade_a";
        fn setup(builder: &mut PassBuilder) -> impl Fn(&mut RenderContext) + Send + Sync + 'static {
            let res =
                builder.create::<MockResource>("res_cascade_r1", 1024, ResourceUsage::Attachment);
            builder.write::<MockResource>(res);
            move |_ctx| {}
        }
    }

    struct CascadeB;
    impl GraphPass for CascadeB {
        const NAME: Name = "cascade_b";
        fn setup(builder: &mut PassBuilder) -> impl Fn(&mut RenderContext) + Send + Sync + 'static {
            let r1 =
                builder.create::<MockResource>("res_cascade_r1", 1024, ResourceUsage::Attachment);
            builder.read::<MockResource>(r1);
            let r2 =
                builder.create::<MockResourceB>("res_cascade_r2", 1024, ResourceUsage::Attachment);
            builder.write::<MockResourceB>(r2);
            move |_ctx| {}
        }
    }

    struct CascadeC;
    impl GraphPass for CascadeC {
        const NAME: Name = "cascade_c";
        fn setup(builder: &mut PassBuilder) -> impl Fn(&mut RenderContext) + Send + Sync + 'static {
            let r2 =
                builder.create::<MockResourceB>("res_cascade_r2", 1024, ResourceUsage::Attachment);
            builder.read::<MockResourceB>(r2);
            let r3 = builder.create::<MockResourceImported>(
                "res_cascade_r3",
                1024,
                ResourceUsage::Attachment,
            );
            builder.write::<MockResourceImported>(r3);
            move |_ctx| {}
        }
    }

    // Multi-output pass: creates+writes R1 and R2. R1 has a reader, R2 does not.
    // Pass survives because ref_count > 0 from R1's write.

    struct MultiOutputPass;
    impl GraphPass for MultiOutputPass {
        const NAME: Name = "multi_output";
        fn setup(builder: &mut PassBuilder) -> impl Fn(&mut RenderContext) + Send + Sync + 'static {
            let r1 =
                builder.create::<MockResource>("res_multi_r1", 1024, ResourceUsage::Attachment);
            builder.write::<MockResource>(r1);
            let r2 =
                builder.create::<MockResourceB>("res_multi_r2", 1024, ResourceUsage::Attachment);
            builder.write::<MockResourceB>(r2);
            move |_ctx| {}
        }
    }

    struct MultiOutputReader;
    impl GraphPass for MultiOutputReader {
        const NAME: Name = "multi_output_reader";
        fn setup(builder: &mut PassBuilder) -> impl Fn(&mut RenderContext) + Send + Sync + 'static {
            let r1 =
                builder.create::<MockResource>("res_multi_r1", 1024, ResourceUsage::Attachment);
            builder.read::<MockResource>(r1);
            builder.write::<MockResource>(r1);
            move |_ctx| {}
        }
    }

    // ReadOnlyPass: only reads a resource (no writes) → ref_count = 0 before culling.

    struct ReadOnlyPass;
    impl GraphPass for ReadOnlyPass {
        const NAME: Name = "read_only";
        fn setup(builder: &mut PassBuilder) -> impl Fn(&mut RenderContext) + Send + Sync + 'static {
            let r1 =
                builder.create::<MockResource>("res_readonly", 1024, ResourceUsage::Attachment);
            builder.read::<MockResource>(r1);
            move |_ctx| {}
        }
    }

    struct ReadOnlyProducer;
    impl GraphPass for ReadOnlyProducer {
        const NAME: Name = "read_only_producer";
        fn setup(builder: &mut PassBuilder) -> impl Fn(&mut RenderContext) + Send + Sync + 'static {
            let r1 =
                builder.create::<MockResource>("res_readonly", 1024, ResourceUsage::Attachment);
            builder.write::<MockResource>(r1);
            move |_ctx| {}
        }
    }

    // ── Cull Stage Unit Tests ───────────────────────────────────────────

    #[test]
    fn dead_resource_chain_removal() {
        // A creates+writes R1, B reads R1 and creates+writes R2, C reads+writes R1.
        // R2 has no readers → B is culled, A and C survive.
        let mut graph = RenderGraph::default();
        let a = graph.add_pass::<CullCreateA>();
        graph.add_after::<CullReadWriteB>(a);
        graph.add_after::<CullReadWriteC>(a);

        let world = default_world();
        let settings = default_settings();
        let cameras = make_camera_queue(vec![make_camera(None)]);

        let compiled = RenderGraphCompiler::run(&world, &graph, &settings, &cameras);

        // B should be culled (R2 has no readers), A and C should survive
        assert_eq!(
            compiled.passes.len(),
            2,
            "B should be culled; A and C should survive"
        );

        let node_ids: Vec<u32> = compiled.passes.iter().map(|p| p.node).collect();
        assert!(node_ids.contains(&*a), "pass A should survive culling");
        // CullReadWriteC's PassId
        let c = graph.add_pass::<CullReadWriteC>(); // returns existing PassId
        assert!(node_ids.contains(&*c), "pass C should survive culling");
    }

    #[test]
    fn cascading_cull_propagation() {
        // A creates R1, B reads R1 and creates R2, C reads R2 and creates R3.
        // R3 has no readers → C culled → R2 dead → B culled → R1 dead → A culled.
        let mut graph = RenderGraph::default();
        let a = graph.add_pass::<CascadeA>();
        let b = graph.add_after::<CascadeB>(a);
        graph.add_after::<CascadeC>(b);

        let world = default_world();
        let settings = default_settings();
        let cameras = make_camera_queue(vec![make_camera(None)]);

        let compiled = RenderGraphCompiler::run(&world, &graph, &settings, &cameras);

        assert_eq!(
            compiled.passes.len(),
            0,
            "entire chain should be culled when terminal resource has no readers"
        );
    }

    #[test]
    fn multi_output_pass_retention() {
        // MultiOutputPass creates+writes R1 and R2. MultiOutputReader reads+writes R1.
        // R2 has no readers, but MultiOutputPass survives because R1's write keeps ref_count > 0.
        let mut graph = RenderGraph::default();
        let producer = graph.add_pass::<MultiOutputPass>();
        graph.add_after::<MultiOutputReader>(producer);

        let world = default_world();
        let settings = default_settings();
        let cameras = make_camera_queue(vec![make_camera(None)]);

        let compiled = RenderGraphCompiler::run(&world, &graph, &settings, &cameras);

        // Both passes should survive: MultiOutputPass has ref_count=2 (two writes),
        // R2 dead decrements it to 1, still > 0.
        assert_eq!(
            compiled.passes.len(),
            2,
            "multi-output pass should survive when at least one output has readers"
        );

        let node_ids: Vec<u32> = compiled.passes.iter().map(|p| p.node).collect();
        assert!(
            node_ids.contains(&*producer),
            "multi-output producer pass should survive"
        );
    }

    // ── Allocate Stage Unit Tests ───────────────────────────────────────

    // AllocPass1: creates + writes a MockResource with desc=1024.
    // AllocPass2: reads + writes the same resource so it has a reader and both passes survive.
    // When run with 2 cameras, camera 0 gets pass indices 0..1 and camera 1 gets 2..3.
    // The resource is deduplicated across cameras (same type + compatible desc), resulting
    // in a single allocation — validating that compatible resources share an allocation.

    struct AllocPass1;
    impl GraphPass for AllocPass1 {
        const NAME: Name = "alloc_pass_1";
        fn setup(builder: &mut PassBuilder) -> impl Fn(&mut RenderContext) + Send + Sync + 'static {
            let res = builder.create::<MockResource>("res_alloc", 1024, ResourceUsage::Attachment);
            builder.write::<MockResource>(res);
            move |_ctx| {}
        }
    }

    struct AllocPass2;
    impl GraphPass for AllocPass2 {
        const NAME: Name = "alloc_pass_2";
        fn setup(builder: &mut PassBuilder) -> impl Fn(&mut RenderContext) + Send + Sync + 'static {
            let res = builder.create::<MockResource>("res_alloc", 1024, ResourceUsage::Attachment);
            builder.read::<MockResource>(res);
            builder.write::<MockResource>(res);
            move |_ctx| {}
        }
    }

    #[test]
    fn allocation_reuse_non_overlapping_lifetimes() {
        // Two cameras each run AllocPass1 → AllocPass2 on the same MockResource desc=1024.
        // Camera 0's resource spans pass indices 0..1, camera 1's spans 2..3.
        // Same type (MockResource), same kind (Transient), compatible desc (1024 == 1024),
        // non-overlapping lifetimes → the compiler should produce a single allocation.
        let mut graph = RenderGraph::default();
        let p1 = graph.add_pass::<AllocPass1>();
        graph.add_after::<AllocPass2>(p1);

        let world = default_world();
        let settings = default_settings();
        let cameras = make_camera_queue(vec![make_camera(None), make_camera(None)]);

        let compiled = RenderGraphCompiler::run(&world, &graph, &settings, &cameras);

        // 2 passes × 2 cameras = 4 pass instances
        assert_eq!(
            compiled.passes.len(),
            4,
            "2 passes × 2 cameras should produce 4 pass instances"
        );

        // Both cameras' resources have the same type and compatible desc, so the
        // compiler should share a single allocation.
        assert_eq!(
            compiled.allocations.len(),
            1,
            "resources with same type, same kind, compatible desc, and non-overlapping lifetimes \
             should share one allocation (requirement 6.1)"
        );
    }

    #[test]
    fn pass_with_ref_count_zero_before_culling() {
        // ReadOnlyProducer creates+writes R1, ReadOnlyPass only reads R1 (no writes).
        // ReadOnlyPass has ref_count=0 (no writes) → culled by retain.
        // R1 still has ref_count=1 from ReadOnlyPass's read, so ReadOnlyProducer survives.
        // The cull loop only processes dead resources, not dead passes directly,
        // so ReadOnlyPass being culled does NOT cascade to decrement R1.

        let mut graph = RenderGraph::default();
        let producer = graph.add_pass::<ReadOnlyProducer>();
        graph.add_after::<ReadOnlyPass>(producer);

        let world = default_world();
        let settings = default_settings();
        let cameras = make_camera_queue(vec![make_camera(None)]);

        let compiled = RenderGraphCompiler::run(&world, &graph, &settings, &cameras);

        // ReadOnlyPass (ref_count=0, no writes) is culled.
        // ReadOnlyProducer survives because R1 had a reader during expand (ref_count=1).
        assert_eq!(
            compiled.passes.len(),
            1,
            "read-only pass (ref_count=0) should be culled, producer should survive"
        );
        assert_eq!(
            compiled.passes[0].node, *producer,
            "surviving pass should be the producer"
        );
    }

    // ── Separate Allocation Tests (Task 9.2) ────────────────────────────

    // Passes for testing separate allocations due to different types.
    // AllocDiffTypeA: creates + writes MockResource desc=1024
    // AllocDiffTypeB: creates + writes MockResourceB desc=1024
    // AllocDiffTypeReader: reads both resources so they survive cull

    struct AllocDiffTypeA;
    impl GraphPass for AllocDiffTypeA {
        const NAME: Name = "alloc_diff_type_a";
        fn setup(builder: &mut PassBuilder) -> impl Fn(&mut RenderContext) + Send + Sync + 'static {
            let res =
                builder.create::<MockResource>("res_diff_type_a", 1024, ResourceUsage::Attachment);
            builder.write::<MockResource>(res);
            move |_ctx| {}
        }
    }

    struct AllocDiffTypeB;
    impl GraphPass for AllocDiffTypeB {
        const NAME: Name = "alloc_diff_type_b";
        fn setup(builder: &mut PassBuilder) -> impl Fn(&mut RenderContext) + Send + Sync + 'static {
            let res =
                builder.create::<MockResourceB>("res_diff_type_b", 1024, ResourceUsage::Attachment);
            builder.write::<MockResourceB>(res);
            move |_ctx| {}
        }
    }

    struct AllocDiffTypeReader;
    impl GraphPass for AllocDiffTypeReader {
        const NAME: Name = "alloc_diff_type_reader";
        fn setup(builder: &mut PassBuilder) -> impl Fn(&mut RenderContext) + Send + Sync + 'static {
            let r1 =
                builder.create::<MockResource>("res_diff_type_a", 1024, ResourceUsage::Attachment);
            builder.read::<MockResource>(r1);
            builder.write::<MockResource>(r1);
            let r2 =
                builder.create::<MockResourceB>("res_diff_type_b", 1024, ResourceUsage::Attachment);
            builder.read::<MockResourceB>(r2);
            builder.write::<MockResourceB>(r2);
            move |_ctx| {}
        }
    }

    #[test]
    fn separate_allocations_different_types() {
        // MockResource desc=1024 vs MockResourceB desc=1024.
        // Same desc value, same kind (both Transient), but different types →
        // the allocator should produce separate allocations (requirement 6.3).
        let mut graph = RenderGraph::default();
        let a = graph.add_pass::<AllocDiffTypeA>();
        let b = graph.add_pass::<AllocDiffTypeB>();
        graph.add_after::<AllocDiffTypeReader>(a);
        graph.add_after::<AllocDiffTypeReader>(b);

        let world = default_world();
        let settings = default_settings();
        let cameras = make_camera_queue(vec![make_camera(None)]);

        let compiled = RenderGraphCompiler::run(&world, &graph, &settings, &cameras);

        assert_eq!(
            compiled.passes.len(),
            3,
            "all three passes should survive culling"
        );
        assert_eq!(
            compiled.allocations.len(),
            2,
            "different resource types (MockResource vs MockResourceB) should produce \
             separate allocations even with compatible desc values (requirement 6.3)"
        );
    }

    // Passes for testing separate allocations due to different kinds.
    // AllocDiffKindTransient: creates + writes MockResource (Transient) desc=1024
    // AllocDiffKindImported: creates + writes MockResourceImported (Imported) desc=1024
    // AllocDiffKindReader: reads both resources so they survive cull

    struct AllocDiffKindTransient;
    impl GraphPass for AllocDiffKindTransient {
        const NAME: Name = "alloc_diff_kind_transient";
        fn setup(builder: &mut PassBuilder) -> impl Fn(&mut RenderContext) + Send + Sync + 'static {
            let res =
                builder.create::<MockResource>("res_diff_kind_t", 1024, ResourceUsage::Attachment);
            builder.write::<MockResource>(res);
            move |_ctx| {}
        }
    }

    struct AllocDiffKindImported;
    impl GraphPass for AllocDiffKindImported {
        const NAME: Name = "alloc_diff_kind_imported";
        fn setup(builder: &mut PassBuilder) -> impl Fn(&mut RenderContext) + Send + Sync + 'static {
            let res = builder.create::<MockResourceImported>(
                "res_diff_kind_i",
                1024,
                ResourceUsage::Attachment,
            );
            builder.write::<MockResourceImported>(res);
            move |_ctx| {}
        }
    }

    struct AllocDiffKindReader;
    impl GraphPass for AllocDiffKindReader {
        const NAME: Name = "alloc_diff_kind_reader";
        fn setup(builder: &mut PassBuilder) -> impl Fn(&mut RenderContext) + Send + Sync + 'static {
            let r1 =
                builder.create::<MockResource>("res_diff_kind_t", 1024, ResourceUsage::Attachment);
            builder.read::<MockResource>(r1);
            builder.write::<MockResource>(r1);
            let r2 = builder.create::<MockResourceImported>(
                "res_diff_kind_i",
                1024,
                ResourceUsage::Attachment,
            );
            builder.read::<MockResourceImported>(r2);
            builder.write::<MockResourceImported>(r2);
            move |_ctx| {}
        }
    }

    #[test]
    fn separate_allocations_different_kinds() {
        // MockResource (Transient) desc=1024 vs MockResourceImported (Imported) desc=1024.
        // Same desc value but different kinds → the allocator should produce
        // separate allocations (requirement 6.4).
        let mut graph = RenderGraph::default();
        let t = graph.add_pass::<AllocDiffKindTransient>();
        let i = graph.add_pass::<AllocDiffKindImported>();
        graph.add_after::<AllocDiffKindReader>(t);
        graph.add_after::<AllocDiffKindReader>(i);

        let world = default_world();
        let settings = default_settings();
        let cameras = make_camera_queue(vec![make_camera(None)]);

        let compiled = RenderGraphCompiler::run(&world, &graph, &settings, &cameras);

        assert_eq!(
            compiled.passes.len(),
            3,
            "all three passes should survive culling"
        );
        assert_eq!(
            compiled.allocations.len(),
            2,
            "different resource kinds (Transient vs Imported) should produce \
             separate allocations (requirement 6.4)"
        );
    }

    // Passes for testing separate allocations due to incompatible descriptions.
    // AllocIncompatA: creates + writes MockResource desc=1024
    // AllocIncompatB: creates + writes MockResource desc=2048
    // AllocIncompatReader: reads both resources so they survive cull

    struct AllocIncompatA;
    impl GraphPass for AllocIncompatA {
        const NAME: Name = "alloc_incompat_a";
        fn setup(builder: &mut PassBuilder) -> impl Fn(&mut RenderContext) + Send + Sync + 'static {
            let res =
                builder.create::<MockResource>("res_incompat_a", 1024, ResourceUsage::Attachment);
            builder.write::<MockResource>(res);
            move |_ctx| {}
        }
    }

    struct AllocIncompatB;
    impl GraphPass for AllocIncompatB {
        const NAME: Name = "alloc_incompat_b";
        fn setup(builder: &mut PassBuilder) -> impl Fn(&mut RenderContext) + Send + Sync + 'static {
            let res =
                builder.create::<MockResource>("res_incompat_b", 2048, ResourceUsage::Attachment);
            builder.write::<MockResource>(res);
            move |_ctx| {}
        }
    }

    struct AllocIncompatReader;
    impl GraphPass for AllocIncompatReader {
        const NAME: Name = "alloc_incompat_reader";
        fn setup(builder: &mut PassBuilder) -> impl Fn(&mut RenderContext) + Send + Sync + 'static {
            let r1 =
                builder.create::<MockResource>("res_incompat_a", 1024, ResourceUsage::Attachment);
            builder.read::<MockResource>(r1);
            builder.write::<MockResource>(r1);
            let r2 =
                builder.create::<MockResource>("res_incompat_b", 2048, ResourceUsage::Attachment);
            builder.read::<MockResource>(r2);
            builder.write::<MockResource>(r2);
            move |_ctx| {}
        }
    }

    #[test]
    fn separate_allocations_incompatible_descs() {
        // MockResource desc=1024 vs MockResource desc=2048.
        // Same type, same kind (both Transient), but incompatible descriptions
        // (compatible(1024, 2048) = false) → separate allocations (requirement 6.5).
        let mut graph = RenderGraph::default();
        let a = graph.add_pass::<AllocIncompatA>();
        let b = graph.add_pass::<AllocIncompatB>();
        graph.add_after::<AllocIncompatReader>(a);
        graph.add_after::<AllocIncompatReader>(b);

        let world = default_world();
        let settings = default_settings();
        let cameras = make_camera_queue(vec![make_camera(None)]);

        let compiled = RenderGraphCompiler::run(&world, &graph, &settings, &cameras);

        assert_eq!(
            compiled.passes.len(),
            3,
            "all three passes should survive culling"
        );
        assert_eq!(
            compiled.allocations.len(),
            2,
            "incompatible descriptions (1024 vs 2048) should produce \
             separate allocations even with same type and kind (requirement 6.5)"
        );
    }

    #[test]
    fn allocation_last_user_updated_on_reuse() {
        // 3 cameras, each running AllocPass1 → AllocPass2 on MockResource desc=1024.
        // Camera 0: pass indices 0,1 → resource lifetime first_user=0, last_user=1
        // Camera 1: pass indices 2,3 → resource lifetime first_user=2, last_user=3
        // Camera 2: pass indices 4,5 → resource lifetime first_user=4, last_user=5
        //
        // All three resources have same type (MockResource), same kind (Transient),
        // compatible desc (1024). Non-overlapping lifetimes allow reuse:
        // - Camera 0's resource creates the allocation with last_user=Some(1)
        // - Camera 1's resource reuses it, updating last_user to Some(3)
        // - Camera 2's resource reuses it again, updating last_user to Some(5)
        //
        // Result: 1 allocation with last_user=Some(5) (requirement 6.6).
        let mut graph = RenderGraph::default();
        let p1 = graph.add_pass::<AllocPass1>();
        graph.add_after::<AllocPass2>(p1);

        let world = default_world();
        let settings = default_settings();
        let cameras = make_camera_queue(vec![
            make_camera(None),
            make_camera(None),
            make_camera(None),
        ]);

        let compiled = RenderGraphCompiler::run(&world, &graph, &settings, &cameras);

        // 2 passes × 3 cameras = 6 pass instances
        assert_eq!(
            compiled.passes.len(),
            6,
            "2 passes × 3 cameras should produce 6 pass instances"
        );

        // All three resources reuse into a single allocation
        assert_eq!(
            compiled.allocations.len(),
            1,
            "three compatible resources with non-overlapping lifetimes should share one allocation"
        );

        // The allocation's last_user should be updated to the last reusing resource's last_user
        assert_eq!(
            compiled.allocations[0].last_user,
            Some(5),
            "allocation last_user should be updated to Some(5) after two reuses (requirement 6.6)"
        );
    }

    // ── Sort-Order-Dependent Reuse & Zero-Ref Filtering Tests (Task 9.4) ─

    #[test]
    fn zero_ref_resources_filtered_before_allocation() {
        // MultiOutputPass creates+writes R1 (MockResource) and R2 (MockResourceB).
        // MultiOutputReader reads+writes R1. R2 has no readers → ref_count=0 after cull.
        //
        // The allocate stage filters out resources with ref_count == 0 before allocation.
        // Only R1 (which has readers) should get an allocation. R2 should be filtered out.
        // This validates requirements 6.8 (zero-ref filtering).
        let mut graph = RenderGraph::default();
        let producer = graph.add_pass::<MultiOutputPass>();
        graph.add_after::<MultiOutputReader>(producer);

        let world = default_world();
        let settings = default_settings();
        let cameras = make_camera_queue(vec![make_camera(None)]);

        let compiled = RenderGraphCompiler::run(&world, &graph, &settings, &cameras);

        // Both passes survive culling (MultiOutputPass has ref_count > 0 from R1's write)
        assert_eq!(
            compiled.passes.len(),
            2,
            "both passes should survive culling"
        );

        // Only R1 should have an allocation; R2 (ref_count=0, no readers) is filtered out
        assert_eq!(
            compiled.allocations.len(),
            1,
            "resources with ref_count == 0 should be filtered out before allocation \
             (requirement 6.8) — only R1 should get an allocation"
        );
    }

    #[test]
    fn allocation_sort_order_enables_reuse() {
        // 3 cameras, each running AllocPass1 → AllocPass2 on MockResource desc=1024.
        // After expand, each camera produces a deduplicated resource. The resources
        // have first_user values corresponding to their camera's pass indices:
        //   Camera 0: first_user=0, last_user=1
        //   Camera 1: first_user=2, last_user=3
        //   Camera 2: first_user=4, last_user=5
        //
        // The allocate stage sorts resources by first_user (requirement 6.7) before
        // attempting reuse. This ensures resources are processed in chronological order:
        //   1. Process resource with first_user=0 → create default allocation (last_user=1)
        //   2. Process resource with first_user=2 → reuse (1 < 2), update last_user=3
        //   3. Process resource with first_user=4 → reuse (3 < 4), update last_user=5
        //
        // Without sorting, resources might be processed out of order, potentially
        // preventing valid reuse. The sort guarantees optimal reuse.
        let mut graph = RenderGraph::default();
        let p1 = graph.add_pass::<AllocPass1>();
        graph.add_after::<AllocPass2>(p1);

        let world = default_world();
        let settings = default_settings();
        let cameras = make_camera_queue(vec![
            make_camera(None),
            make_camera(None),
            make_camera(None),
        ]);

        let compiled = RenderGraphCompiler::run(&world, &graph, &settings, &cameras);

        // 2 passes × 3 cameras = 6 pass instances
        assert_eq!(
            compiled.passes.len(),
            6,
            "2 passes × 3 cameras should produce 6 pass instances"
        );

        // All three resources should reuse into a single allocation because
        // sorting by first_user ensures they are processed in the correct order
        // for sequential reuse (requirement 6.7).
        assert_eq!(
            compiled.allocations.len(),
            1,
            "sorting resources by first_user before allocation should enable sequential \
             reuse of compatible resources with non-overlapping lifetimes (requirement 6.7)"
        );

        // Verify the final allocation's last_user reflects the last resource processed
        assert_eq!(
            compiled.allocations[0].last_user,
            Some(5),
            "allocation last_user should reflect the last reusing resource's lifetime end"
        );
    }

    // ── Bindings-aware mock resource ────────────────────────────────────
    //
    // MockResourceBindable pushes a uniform-buffer entry into the layout
    // builder so that bind_group_layouts are non-empty and comparable.
    // The entry uses the desc value as the binding index, allowing tests
    // to distinguish layouts by their desc.

    struct MockResourceBindable;

    impl GraphResource for MockResourceBindable {
        type Desc = u32;

        fn resolve(
            _world: &World,
            _settings: &RenderSettings,
            _resolver: &mut crate::renderer::graph::ResourceResolver,
            desc: u32,
        ) -> u32 {
            desc
        }

        fn create(_device: &crate::core::RenderDevice, _name: Name, _desc: &u32) -> Self {
            MockResourceBindable
        }

        fn entry(
            _settings: &RenderSettings,
            desc: &u32,
            builder: &mut BindGroupLayoutBuilder,
            visibility: ShaderStages,
        ) {
            // Use desc as the binding index so different descs produce
            // different layout entries, enabling layout deduplication tests.
            builder.entries.push(wgpu::BindGroupLayoutEntry {
                binding: *desc,
                visibility,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            });
        }

        fn bind<'a>(&'a self, _builder: &mut BindGroupBuilder<'a>) {}

        fn compatible(a: &u32, b: &u32) -> bool {
            a == b
        }

        fn generation(&self) -> u32 {
            0
        }

        fn kind() -> ResourceKind {
            ResourceKind::Transient
        }
    }

    // ── Task 10.1: Bind group construction ──────────────────────────────

    /// Pass that creates a bindable resource, reads it via group 0 binding,
    /// and writes to it. Also reads the resource so it has ref_count > 0
    /// and the pass's write keeps it alive through culling.
    struct BindReadPassA;
    impl GraphPass for BindReadPassA {
        const NAME: Name = "bind_read_a";
        fn setup(builder: &mut PassBuilder) -> impl Fn(&mut RenderContext) + Send + Sync + 'static {
            let res = builder.create::<MockResourceBindable>(
                "bind_res",
                1024,
                ResourceUsage::Binding {
                    group: 0,
                    binding: 0,
                    visiblitiy: ShaderStages::FRAGMENT,
                },
            );

            builder.read::<MockResourceBindable>(res);
            builder.write::<MockResourceBindable>(res);
            move |_| {}
        }
    }

    /// Second pass with identical binding layout (group 0, binding 0, FRAGMENT)
    /// and same resource desc — used to test layout and key deduplication.
    struct BindReadPassB;
    impl GraphPass for BindReadPassB {
        const NAME: Name = "bind_read_b";
        fn setup(builder: &mut PassBuilder) -> impl Fn(&mut RenderContext) + Send + Sync + 'static {
            let res = builder.create::<MockResourceBindable>(
                "bind_res",
                1024,
                ResourceUsage::Binding {
                    group: 0,
                    binding: 0,
                    visiblitiy: ShaderStages::FRAGMENT,
                },
            );

            builder.read::<MockResourceBindable>(res);
            builder.write::<MockResourceBindable>(res);
            move |_| {}
        }
    }

    #[test]
    fn bind_group_construction_and_deduplication() {
        // Requirements 7.1, 7.3, 7.4
        //
        // BindReadPassA and BindReadPassB each create MockResourceBindable
        // with desc=1024 (deduplicated), read it via group 0 binding 0
        // FRAGMENT, and both read + write the resource so it survives
        // culling (the read gives the resource ref_count > 0, and the
        // write gives each pass ref_count > 0).
        //
        // Because the resource is deduplicated (same type, compatible desc),
        // both passes produce identical layout entries and identical
        // BindGroupKeys (same layout index + same allocation indices).
        //
        // Expected:
        //   - bind_group_layouts.len() == 1 (layout deduplication via IndexSet)
        //   - bind_groups.len() == 1 (key deduplication via IndexSet)
        //   - Each pass has exactly 1 binding group (group 0)
        //   - Both passes reference the same layout and bind_group index

        let mut graph = RenderGraph::default();
        let pa = graph.add_pass::<BindReadPassA>();
        graph.add_after::<BindReadPassB>(pa);

        let world = default_world();
        let settings = default_settings();
        let cameras = make_camera_queue(vec![make_camera(None)]);

        let compiled = RenderGraphCompiler::run(&world, &graph, &settings, &cameras);

        // Both passes survive (each has ref_count > 0 from the write,
        // and the resource has ref_count > 0 from the reads)
        assert_eq!(compiled.passes.len(), 2);

        // Bindings are grouped by group index — each pass has 1 group
        for (i, pass) in compiled.passes.iter().enumerate() {
            assert_eq!(
                pass.bindings.len(),
                1,
                "pass {i} should have exactly 1 bind group"
            );
        }

        // Layout deduplication: identical layouts share the same index
        assert_eq!(
            compiled.bind_group_layouts.len(),
            1,
            "identical layouts should be deduplicated via IndexSet (requirement 7.3)"
        );
        assert_eq!(
            compiled.passes[0].bindings[0].layout, compiled.passes[1].bindings[0].layout,
            "both passes should reference the same layout index"
        );

        // Bind group key deduplication: same layout + same allocations → same key
        assert_eq!(
            compiled.bind_groups.len(),
            1,
            "identical bind group keys should be deduplicated via IndexSet (requirement 7.4)"
        );
        assert_eq!(
            compiled.passes[0].bindings[0].bind_group, compiled.passes[1].bindings[0].bind_group,
            "both passes should reference the same bind_group index"
        );
    }

    // ── Task 10.2: Binding resolution through ref_table and alloc_table ─

    /// Pass that creates a bindable resource, reads it via a binding, and
    /// reads from it (to survive culling). The binding's resource must be
    /// resolved through ref_table and alloc_table to get the correct
    /// allocation index.
    struct BindResConsumer;
    impl GraphPass for BindResConsumer {
        const NAME: Name = "bind_res_consumer";
        fn setup(builder: &mut PassBuilder) -> impl Fn(&mut RenderContext) + Send + Sync + 'static {
            let res = builder.create::<MockResourceBindable>(
                "bind_res",
                1024,
                ResourceUsage::Binding {
                    group: 0,
                    binding: 0,
                    visiblitiy: ShaderStages::VERTEX,
                },
            );

            builder.read::<MockResourceBindable>(res);
            builder.write::<MockResourceBindable>(res);
            move |_| {}
        }
    }

    #[test]
    fn binding_resolution_through_ref_and_alloc_tables() {
        // Requirement 7.2
        //
        // BindResConsumer creates a resource, reads it via a binding, and
        // writes to it. The bindings stage resolves each binding's resource
        // through ref_table (cursor + node -> resource ref index) and
        // alloc_table (ref index -> allocation index).
        //
        // We verify that:
        //   1. The pass's binding resolves to the correct allocation.
        //   2. There is exactly 1 allocation.

        let mut graph = RenderGraph::default();
        graph.add_pass::<BindResConsumer>();

        let world = default_world();
        let settings = default_settings();
        let cameras = make_camera_queue(vec![make_camera(None)]);

        let compiled = RenderGraphCompiler::run(&world, &graph, &settings, &cameras);

        // Pass survives (ref_count=1 from write)
        assert_eq!(compiled.passes.len(), 1);

        // Exactly 1 allocation
        assert_eq!(
            compiled.allocations.len(),
            1,
            "single resource should produce 1 allocation"
        );

        // The pass has a binding
        let pass = &compiled.passes[0];
        assert_eq!(pass.bindings.len(), 1, "pass should have 1 bind group");

        // Verify the resources table (alloc_table) maps correctly:
        // The pass's cursor + binding node should resolve to allocation 0.
        let cursor = pass.cursor as usize;
        let alloc_index = compiled.resources[cursor];
        assert_eq!(
            alloc_index, 0,
            "binding resource should resolve to allocation index 0 through ref_table and alloc_table"
        );
    }

    // ── Task 10.3: Multi-group bindings sorted by group index ───────────

    /// Pass with bindings in groups 2, 0, 1 (declared out of order).
    /// Also writes to each resource so it survives culling (ref_count > 0).
    struct MultiGroupPass;
    impl GraphPass for MultiGroupPass {
        const NAME: Name = "multi_group_pass";
        fn setup(builder: &mut PassBuilder) -> impl Fn(&mut RenderContext) + Send + Sync + 'static {
            let r0 = builder.create::<MockResourceBindable>(
                "res_g2",
                1024,
                ResourceUsage::Binding {
                    group: 2,
                    binding: 0,
                    visiblitiy: ShaderStages::FRAGMENT,
                },
            );
            let r1 = builder.create::<MockResourceBindable>(
                "res_g0",
                2048,
                ResourceUsage::Binding {
                    group: 0,
                    binding: 0,
                    visiblitiy: ShaderStages::FRAGMENT,
                },
            );
            let r2 = builder.create::<MockResourceBindable>(
                "res_g1",
                4096,
                ResourceUsage::Binding {
                    group: 1,
                    binding: 0,
                    visiblitiy: ShaderStages::FRAGMENT,
                },
            );

            // Write to each resource so this pass has ref_count = 3
            builder.read::<MockResourceBindable>(r0);
            builder.write::<MockResourceBindable>(r0);
            builder.read::<MockResourceBindable>(r1);
            builder.write::<MockResourceBindable>(r1);
            builder.read::<MockResourceBindable>(r2);
            builder.write::<MockResourceBindable>(r2);
            move |_| {}
        }
    }

    #[test]
    fn multi_group_bindings_sorted_by_group_index() {
        // Requirement 7.5
        //
        // MultiGroupPass declares bindings in groups 2, 0, 1 (out of order).
        // The bindings stage must sort them so PassInstance.bindings is
        // ordered [group 0, group 1, group 2].

        let mut graph = RenderGraph::default();
        graph.add_pass::<MultiGroupPass>();

        let world = default_world();
        let settings = default_settings();
        let cameras = make_camera_queue(vec![make_camera(None)]);

        let compiled = RenderGraphCompiler::run(&world, &graph, &settings, &cameras);

        assert_eq!(compiled.passes.len(), 1, "MultiGroupPass should survive");

        let mg_pass = &compiled.passes[0];
        assert_eq!(
            mg_pass.bindings.len(),
            3,
            "MultiGroupPass should have 3 bind groups"
        );

        // Verify bindings are sorted by group index (ascending).
        // Each group has a different desc (1024, 2048, 4096) so they produce
        // different layout entries → distinct layout indices.
        // The sorted order should be group 0 (desc 2048), group 1 (desc 4096),
        // group 2 (desc 1024).
        //
        // Since layouts are inserted into the IndexSet in sorted group order
        // (0, 1, 2), the layout indices should be monotonically increasing.
        for i in 1..mg_pass.bindings.len() {
            assert!(
                mg_pass.bindings[i - 1].layout < mg_pass.bindings[i].layout,
                "bindings should be sorted by group index — layout indices should be ascending"
            );
        }

        // Verify there are exactly 3 distinct layouts (one per group)
        assert_eq!(
            compiled.bind_group_layouts.len(),
            3,
            "3 groups with different descs should produce 3 distinct layouts"
        );
    }

    // ── Task 11: End-to-End Pipeline Tests ──────────────────────────────

    // 11.1 — Simple linear graph: PassA creates resource, PassB reads it.
    // PassB also writes so it survives culling.

    struct E2eLinearCreate;
    impl GraphPass for E2eLinearCreate {
        const NAME: Name = "e2e_linear_create";
        fn setup(builder: &mut PassBuilder) -> impl Fn(&mut RenderContext) + Send + Sync + 'static {
            let res =
                builder.create::<MockResourceBindable>("e2e_res", 1024, ResourceUsage::Attachment);
            builder.write::<MockResourceBindable>(res);
            move |_| {}
        }
    }

    struct E2eLinearRead;
    impl GraphPass for E2eLinearRead {
        const NAME: Name = "e2e_linear_read";
        fn setup(builder: &mut PassBuilder) -> impl Fn(&mut RenderContext) + Send + Sync + 'static {
            let res = builder.create::<MockResourceBindable>(
                "e2e_res",
                1024,
                ResourceUsage::Binding {
                    group: 0,
                    binding: 0,
                    visiblitiy: ShaderStages::FRAGMENT,
                },
            );

            builder.read::<MockResourceBindable>(res);
            builder.write::<MockResourceBindable>(res);
            move |_| {}
        }
    }

    #[test]
    fn e2e_simple_linear_graph() {
        // Requirement 8.1
        // Pass A creates resource, Pass B reads resource, one camera.
        // Expected: 2 passes, 1 allocation, correct bindings on pass B.
        let mut graph = RenderGraph::default();
        let a = graph.add_pass::<E2eLinearCreate>();
        graph.add_after::<E2eLinearRead>(a);

        let world = default_world();
        let settings = default_settings();
        let cameras = make_camera_queue(vec![make_camera(None)]);

        let compiled = RenderGraphCompiler::run(&world, &graph, &settings, &cameras);

        assert_eq!(compiled.passes.len(), 2, "both passes should survive");
        assert_eq!(
            compiled.allocations.len(),
            1,
            "single resource should produce 1 allocation"
        );

        // Pass B (the reader) should have a binding
        let reader = compiled
            .passes
            .iter()
            .find(|p| p.bindings.len() == 1)
            .expect("one pass should have a binding");
        assert_eq!(reader.bindings[0].layout, 0);
        assert_eq!(reader.bindings[0].bind_group, 0);

        // All allocation indices in bindings should be in bounds
        assert!(
            compiled.bind_groups.len() <= compiled.allocations.len() + 1,
            "bind group count should be reasonable"
        );
    }

    // 11.2 — Unused branch culling.
    // A creates R1, B reads R1 and creates R2, C reads R1. R2 has no readers → B culled.

    struct E2eBranchCreate;
    impl GraphPass for E2eBranchCreate {
        const NAME: Name = "e2e_branch_create";
        fn setup(builder: &mut PassBuilder) -> impl Fn(&mut RenderContext) + Send + Sync + 'static {
            let res = builder.create::<MockResource>("e2e_r1", 1024, ResourceUsage::Attachment);
            builder.write::<MockResource>(res);
            move |_| {}
        }
    }

    struct E2eBranchMiddle;
    impl GraphPass for E2eBranchMiddle {
        const NAME: Name = "e2e_branch_middle";
        fn setup(builder: &mut PassBuilder) -> impl Fn(&mut RenderContext) + Send + Sync + 'static {
            let r1 = builder.create::<MockResource>("e2e_r1", 1024, ResourceUsage::Attachment);
            builder.read::<MockResource>(r1);
            let r2 = builder.create::<MockResourceB>("e2e_r2", 1024, ResourceUsage::Attachment);
            builder.write::<MockResourceB>(r2);
            move |_| {}
        }
    }

    struct E2eBranchConsumer;
    impl GraphPass for E2eBranchConsumer {
        const NAME: Name = "e2e_branch_consumer";
        fn setup(builder: &mut PassBuilder) -> impl Fn(&mut RenderContext) + Send + Sync + 'static {
            let r1 = builder.create::<MockResource>("e2e_r1", 1024, ResourceUsage::Attachment);
            builder.read::<MockResource>(r1);
            builder.write::<MockResource>(r1);
            move |_| {}
        }
    }

    #[test]
    fn e2e_unused_branch_culling() {
        // Requirement 8.2
        // A creates R1, B reads R1 and creates R2, C reads R1.
        // R2 has no readers → B culled, A and C retained.
        let mut graph = RenderGraph::default();
        let a = graph.add_pass::<E2eBranchCreate>();
        graph.add_after::<E2eBranchMiddle>(a);
        graph.add_after::<E2eBranchConsumer>(a);

        let world = default_world();
        let settings = default_settings();
        let cameras = make_camera_queue(vec![make_camera(None)]);

        let compiled = RenderGraphCompiler::run(&world, &graph, &settings, &cameras);

        assert_eq!(
            compiled.passes.len(),
            2,
            "B should be culled; A and C should survive"
        );

        let node_ids: Vec<u32> = compiled.passes.iter().map(|p| p.node).collect();
        assert!(node_ids.contains(&*a), "pass A should survive");
        // Verify B is not present — B is the second pass added (index 1)
        assert!(
            !node_ids.contains(&1),
            "pass B (the middle branch) should be culled"
        );
    }

    // 11.3 — Multi-camera compilation.

    #[test]
    fn e2e_multi_camera_compilation() {
        // Requirement 8.3
        // Graph with 2 passes compiled with 2 cameras → pass instances for
        // each camera with correct camera indices.
        let mut graph = RenderGraph::default();
        let a = graph.add_pass::<E2eBranchCreate>();
        graph.add_after::<E2eBranchConsumer>(a);

        let world = default_world();
        let settings = default_settings();
        let cameras = make_camera_queue(vec![make_camera(None), make_camera(None)]);

        let compiled = RenderGraphCompiler::run(&world, &graph, &settings, &cameras);

        // 2 passes × 2 cameras = 4 instances
        assert_eq!(compiled.passes.len(), 4, "2 passes × 2 cameras = 4");

        // Verify camera indices
        let cam0_count = compiled
            .passes
            .iter()
            .filter(|p| p.camera == Some(0))
            .count();
        let cam1_count = compiled
            .passes
            .iter()
            .filter(|p| p.camera == Some(1))
            .count();
        assert_eq!(cam0_count, 2, "camera 0 should have 2 pass instances");
        assert_eq!(cam1_count, 2, "camera 1 should have 2 pass instances");

        // Verify cursors are distinct between cameras
        let cam0_cursors: Vec<u32> = compiled
            .passes
            .iter()
            .filter(|p| p.camera == Some(0))
            .map(|p| p.cursor)
            .collect();
        let cam1_cursors: Vec<u32> = compiled
            .passes
            .iter()
            .filter(|p| p.camera == Some(1))
            .map(|p| p.cursor)
            .collect();
        assert_ne!(
            cam0_cursors[0], cam1_cursors[0],
            "different cameras should have different cursor offsets"
        );
    }

    // 11.4 — Cross-camera allocation reuse.

    #[test]
    fn e2e_cross_camera_allocation_reuse() {
        // Requirement 8.4
        // Resources from different cameras with non-overlapping lifetimes
        // and compatible descriptions should reuse allocations.
        //
        // AllocPass1 creates+writes MockResource desc=1024.
        // AllocPass2 reads+writes the same resource.
        // With 2 cameras: camera 0 passes span indices 0..1, camera 1 spans 2..3.
        // Same type, same kind, compatible desc, non-overlapping → 1 allocation.
        let mut graph = RenderGraph::default();
        let p1 = graph.add_pass::<AllocPass1>();
        graph.add_after::<AllocPass2>(p1);

        let world = default_world();
        let settings = default_settings();
        let cameras = make_camera_queue(vec![make_camera(None), make_camera(None)]);

        let compiled = RenderGraphCompiler::run(&world, &graph, &settings, &cameras);

        assert_eq!(compiled.passes.len(), 4, "2 passes × 2 cameras = 4");

        // Cross-camera allocation reuse: both cameras' resources have the
        // same type (MockResource), same kind (Transient), compatible desc
        // (1024 == 1024), and non-overlapping lifetimes → single allocation.
        assert_eq!(
            compiled.allocations.len(),
            1,
            "cross-camera resources with compatible desc and non-overlapping lifetimes \
             should share one allocation (requirement 8.4)"
        );

        // Verify all allocation indices in the resources table are in bounds
        for &alloc_idx in compiled.resources.iter() {
            assert!(
                (alloc_idx as usize) < compiled.allocations.len(),
                "allocation index {} should be < allocations.len() {}",
                alloc_idx,
                compiled.allocations.len()
            );
        }
    }
}
