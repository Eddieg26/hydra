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
use ecs::{IndexDag, IndexSet, World, core::ImmutableIndexDag};
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
        let mut dag = IndexDag::new();
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
                                let other_node = graph.resources().node(r.node);
                                let ty = graph.resources().ty(other_node.ty);

                                other_node.ty == node.ty && ty.compatible(&desc, &r.desc)
                            }) {
                                table[resource as usize] = index as u32;
                            } else {
                                table[resource as usize] = resources.len() as u32;
                                let ref_count = if ty.root() { 1 } else { 0 };
                                resources.push(ResourceRef {
                                    id: resources.len() as u32,
                                    node: node.id,
                                    ref_count,
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
                    dead_resources.push_back(resource.id);
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
        self.producer = match self.producer {
            Some(first) => Some(first.min(pass)),
            None => Some(pass),
        };

        self.first_user = match self.first_user {
            Some(first) => Some(first.min(pass)),
            None => Some(pass),
        };

        self.last_user = match self.last_user {
            Some(last) => Some(last.max(pass)),
            None => Some(pass),
        };
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

    // ── Mock Passes ─────────────────────────────────────────────────────

    struct PassA;
    impl GraphPass for PassA {
        const NAME: Name = "pass_a";
        fn setup(builder: &mut PassBuilder) -> impl Fn(&mut RenderContext) + Send + Sync + 'static {
            let _res = builder.create::<MockResource>("res_a", 1024);
            move |_ctx| {}
        }
    }

    struct PassB;
    impl GraphPass for PassB {
        const NAME: Name = "pass_b";
        fn setup(
            _builder: &mut PassBuilder,
        ) -> impl Fn(&mut RenderContext) + Send + Sync + 'static {
            move |_ctx| {}
        }
    }

    struct PassC;
    impl GraphPass for PassC {
        const NAME: Name = "pass_c";
        fn setup(
            _builder: &mut PassBuilder,
        ) -> impl Fn(&mut RenderContext) + Send + Sync + 'static {
            move |_ctx| {}
        }
    }

    struct PassD;
    impl GraphPass for PassD {
        const NAME: Name = "pass_d";
        fn setup(
            _builder: &mut PassBuilder,
        ) -> impl Fn(&mut RenderContext) + Send + Sync + 'static {
            move |_ctx| {}
        }
    }

    // ── Helper Functions ────────────────────────────────────────────────

    fn make_camera(mask: Option<RenderGraphMask>) -> CameraSettings {
        CameraSettings {
            entity: Entity::new(0, 0),
            priority: 0,
            target: None,
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
            let res = builder.create::<MockResource>("res_diamond", 1024);
            builder.write::<MockResource>(res, ResourceUsage::Attachment);
            move |_ctx| {}
        }
    }

    struct DiamondB;
    impl GraphPass for DiamondB {
        const NAME: Name = "diamond_b";
        fn setup(builder: &mut PassBuilder) -> impl Fn(&mut RenderContext) + Send + Sync + 'static {
            let res = builder.create::<MockResource>("res_diamond", 1024);
            builder.read::<MockResource>(res, ResourceUsage::Attachment);
            builder.write::<MockResource>(res, ResourceUsage::Attachment);
            move |_ctx| {}
        }
    }

    struct DiamondC;
    impl GraphPass for DiamondC {
        const NAME: Name = "diamond_c";
        fn setup(builder: &mut PassBuilder) -> impl Fn(&mut RenderContext) + Send + Sync + 'static {
            let res = builder.create::<MockResource>("res_diamond", 1024);
            builder.read::<MockResource>(res, ResourceUsage::Attachment);
            builder.write::<MockResource>(res, ResourceUsage::Attachment);
            move |_ctx| {}
        }
    }

    struct DiamondD;
    impl GraphPass for DiamondD {
        const NAME: Name = "diamond_d";
        fn setup(builder: &mut PassBuilder) -> impl Fn(&mut RenderContext) + Send + Sync + 'static {
            let res = builder.create::<MockResource>("res_diamond", 1024);
            builder.read::<MockResource>(res, ResourceUsage::Attachment);
            builder.write::<MockResource>(res, ResourceUsage::Attachment);
            move |_ctx| {}
        }
    }

    // ── Single Pass Test Pass ───────────────────────────────────────────
    // Creates, reads, and writes a resource so it survives culling alone.

    struct SinglePass;
    impl GraphPass for SinglePass {
        const NAME: Name = "single_pass";
        fn setup(builder: &mut PassBuilder) -> impl Fn(&mut RenderContext) + Send + Sync + 'static {
            let res = builder.create::<MockResource>("res_single", 512);
            builder.read::<MockResource>(res, ResourceUsage::Attachment);
            builder.write::<MockResource>(res, ResourceUsage::Attachment);
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
                    let res = builder.create::<MockResource>("res_dag", 42);
                    builder.read::<MockResource>(res, ResourceUsage::Attachment);
                    builder.write::<MockResource>(res, ResourceUsage::Attachment);
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
            let mut edges = Vec::new();
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
        let mut graph = RenderGraph::new();
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
                0 => { graph.add_after::<DagPass0>(ids[u]); }
                1 => { graph.add_after::<DagPass1>(ids[u]); }
                2 => { graph.add_after::<DagPass2>(ids[u]); }
                3 => { graph.add_after::<DagPass3>(ids[u]); }
                4 => { graph.add_after::<DagPass4>(ids[u]); }
                5 => { graph.add_after::<DagPass5>(ids[u]); }
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
        let mut pos_map: HashMap<u32, usize> = HashMap::new();
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
        let mut graph = RenderGraph::new();
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
        let mut graph = RenderGraph::new();
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
        let graph = RenderGraph::new();

        let world = default_world();
        let settings = default_settings();
        let cameras = make_camera_queue(vec![make_camera(None)]);

        let compiled = RenderGraphCompiler::run(&world, &graph, &settings, &cameras);

        assert!(
            compiled.passes.is_empty(),
            "empty graph should produce zero pass instances"
        );
    }
}
