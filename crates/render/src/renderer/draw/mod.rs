use crate::{
    Color, DepthOutput, ExtractResource, FragmentState, Frustum, Mesh, MeshLayout, PassBuilder,
    PipelineCache, PipelineId, RenderAssets, RenderContext, RenderGraphPass, RenderMesh,
    RenderOutput, RenderPipelineDesc, RenderResource, RenderState, RenderSurface, Shader, SubMesh,
    VertexState,
};
use asset::{AssetId, ErasedId};
use ecs::{
    ArgItem, Component, Entity, IndexMap, ReadOnly, Resource, SystemArg, SystemMeta, World,
    WorldAccess,
    app::Main,
    system::{
        SystemState,
        unlifetime::{Read, SCommands, SQuery, Write},
    },
    world::WorldCell,
};
use math::{Vec3, bounds::Aabb};
use std::{any::TypeId, collections::HashMap, ops::Range};
use transform::GlobalTransform;
use wgpu::{ColorTargetState, VertexFormat, VertexStepMode};

pub mod material;
pub mod model;
pub mod view;

pub use material::*;
pub use model::*;
pub use view::*;

pub trait Draw: Component + Clone {
    type View: View;

    type Model: ModelData;

    type Material: Material;

    const BATCH: bool = true;

    const CULL: bool = false;

    fn material(&self) -> AssetId<Self::Material>;

    fn mesh(&self) -> AssetId<Mesh>;

    fn sub_mesh(&self) -> Option<AssetId<SubMesh>> {
        None
    }

    fn model(&self, transform: &GlobalTransform) -> Self::Model;

    fn primitive_state() -> wgpu::PrimitiveState {
        wgpu::PrimitiveState::default()
    }

    fn formats() -> &'static [VertexFormat];

    fn shader() -> impl Into<AssetId<Shader>>;
}

pub struct DrawTreeNode<D: Draw> {
    bounds: Aabb,
    items: Vec<DrawIndex<D>>,
    depth: u32,
    max_size: usize,
    max_depth: u32,
    children: Option<Box<[DrawTreeNode<D>]>>,
}

impl<D: Draw> DrawTreeNode<D> {
    fn new(bounds: Aabb, depth: u32, max_size: usize, max_depth: u32) -> Self {
        DrawTreeNode {
            bounds,
            items: Vec::new(),
            depth,
            max_size,
            max_depth,
            children: None,
        }
    }

    pub fn items(&self) -> &[DrawIndex<D>] {
        &self.items
    }

    pub fn bounds(&self) -> &Aabb {
        &self.bounds
    }

    pub fn depth(&self) -> u32 {
        self.depth
    }

    pub fn max_size(&self) -> usize {
        self.max_size
    }

    pub fn max_depth(&self) -> u32 {
        self.max_depth
    }

    pub fn children(&self) -> Option<&[DrawTreeNode<D>]> {
        self.children.as_deref()
    }

    pub fn filter(
        &self,
        nodes: &[DrawNode<D>],
        f: impl Fn(&Aabb) -> bool + Copy,
    ) -> Vec<&DrawIndex<D>> {
        let mut results = Vec::new();

        if !f(&self.bounds) {
            return Vec::new();
        }

        for item in &self.items {
            if f(&nodes[item.0].bounds) {
                results.push(item);
            }
        }

        if let Some(children) = &self.children {
            for child in children.iter() {
                results.extend(child.filter(nodes, f));
            }
        }

        return results;
    }

    pub fn insert(&mut self, item: DrawIndex<D>, bounds: Aabb) -> Option<DrawIndex<D>> {
        if !self.bounds.intersects(bounds) {
            return Some(item);
        }

        if self.items.len() < self.max_size || self.depth >= self.max_depth {
            self.items.push(item);
            return None;
        }

        if self.children.is_none() {
            self.split();
        }

        let mut remaining = Vec::new();
        for item in self.items.drain(..) {
            let mut item = Some(item);
            for child in self.children.as_mut().unwrap().iter_mut() {
                let Some(remaining_item) = item.take() else {
                    break;
                };

                if let Some(remaining_item) = child.insert(remaining_item, bounds) {
                    item = Some(remaining_item);
                }
            }

            if let Some(remaining_item) = item {
                remaining.push(remaining_item);
            }
        }

        self.items = remaining;

        None
    }

    fn split(&mut self) {
        let half_size = (self.bounds.max - self.bounds.min) * 0.5;
        let center = self.bounds.min + half_size;

        let children = [
            DrawTreeNode::new(
                Aabb::new(self.bounds.min, center),
                self.depth + 1,
                self.max_size,
                self.max_depth,
            ),
            DrawTreeNode::new(
                Aabb::new(
                    Vec3::new(center.x, self.bounds.min.y, self.bounds.min.z),
                    Vec3::new(self.bounds.max.x, center.y, center.z),
                ),
                self.depth + 1,
                self.max_size,
                self.max_depth,
            ),
            DrawTreeNode::new(
                Aabb::new(
                    Vec3::new(self.bounds.min.x, center.y, self.bounds.min.z),
                    Vec3::new(center.x, self.bounds.max.y, center.z),
                ),
                self.depth + 1,
                self.max_size,
                self.max_depth,
            ),
            DrawTreeNode::new(
                Aabb::new(
                    Vec3::new(center.x, center.y, self.bounds.min.z),
                    Vec3::new(self.bounds.max.x, self.bounds.max.y, center.z),
                ),
                self.depth + 1,
                self.max_size,
                self.max_depth,
            ),
            DrawTreeNode::new(
                Aabb::new(
                    Vec3::new(self.bounds.min.x, self.bounds.min.y, center.z),
                    Vec3::new(center.x, center.y, self.bounds.max.z),
                ),
                self.depth + 1,
                self.max_size,
                self.max_depth,
            ),
            DrawTreeNode::new(
                Aabb::new(
                    Vec3::new(center.x, self.bounds.min.y, center.z),
                    Vec3::new(self.bounds.max.x, center.y, self.bounds.max.z),
                ),
                self.depth + 1,
                self.max_size,
                self.max_depth,
            ),
            DrawTreeNode::new(
                Aabb::new(
                    Vec3::new(self.bounds.min.x, center.y, center.z),
                    Vec3::new(center.x, self.bounds.max.y, self.bounds.max.z),
                ),
                self.depth + 1,
                self.max_size,
                self.max_depth,
            ),
            DrawTreeNode::new(
                Aabb::new(center, self.bounds.max),
                self.depth + 1,
                self.max_size,
                self.max_depth,
            ),
        ];

        self.children = Some(Box::new(children));
    }

    fn clear(&mut self) {
        self.items.clear();
        self.children = None;
    }
}

#[derive(Resource)]
pub struct DrawTree<D: Draw> {
    root: DrawTreeNode<D>,
    nodes: Vec<DrawNode<D>>,
}

impl<D: Draw> DrawTree<D> {
    pub fn new() -> Self {
        Self {
            root: DrawTreeNode::new(
                Aabb::new(Vec3::splat(-10_000.0), Vec3::splat(10_000.0)),
                0,
                100,
                10,
            ),
            nodes: Vec::new(),
        }
    }

    pub fn insert(&mut self, draw: DrawNode<D>) {
        let index = DrawIndex::new(self.nodes.len());
        self.nodes.push(draw);
        if D::CULL {
            let bounds = self.nodes[index.0].bounds;
            if let Some(index) = self.root.insert(index, bounds) {
                self.root.items.push(index);
            }
        } else {
            self.root.items.push(index);
        }
    }

    pub fn nodes(&self) -> &[DrawNode<D>] {
        &self.nodes
    }

    pub fn get(&self, index: DrawIndex<D>) -> &DrawNode<D> {
        &self.nodes[index.0]
    }

    pub fn filter(&self, f: impl Fn(&Aabb) -> bool + Copy) -> Vec<&DrawIndex<D>> {
        self.root.filter(self.nodes(), |bounds| f(bounds))
    }

    pub fn cull(&self, frustum: &Frustum) -> Vec<&DrawIndex<D>> {
        if D::CULL {
            self.filter(|bounds| frustum.intersects_aabb(bounds))
        } else {
            self.root.items().iter().collect()
        }
    }

    fn clear(&mut self) {
        self.root.clear();
        self.nodes.clear();
    }

    pub(crate) fn extract(
        tree: &mut Self,
        query: Main<
            SQuery<(
                Entity,
                &<D::View as View>::Transform,
                &GlobalTransform,
                &D,
                Option<&DisableCulling>,
            )>,
        >,
        meshes: &RenderAssets<RenderMesh>,
    ) {
        for (entity, transform, global_transform, draw, disable_culling) in query.iter() {
            let Some(mesh) = meshes.get(&draw.mesh()) else {
                continue;
            };

            let bounds = if D::CULL && disable_culling.is_none() {
                mesh.bounds().transform_affine(global_transform.get())
            } else {
                Aabb::ZERO
            };

            let draw = DrawNode {
                entity,
                local_transform: *transform,
                global_transform: *global_transform,
                draw: draw.clone(),
                bounds,
            };

            if D::CULL && disable_culling.is_none() {
                tree.insert(draw);
            } else {
                tree.root.items.push(DrawIndex::new(tree.nodes.len()));
                tree.nodes.push(draw);
            }
        }
    }
}

pub struct DrawNode<D: Draw> {
    pub entity: Entity,
    pub local_transform: <D::View as View>::Transform,
    pub global_transform: GlobalTransform,
    pub bounds: Aabb,
    draw: D,
}

impl<D: Draw> DrawNode<D> {
    pub fn data(&self) -> D::Model {
        self.draw.model(&self.global_transform)
    }
}

impl<D: Draw> AsRef<D> for DrawNode<D> {
    fn as_ref(&self) -> &D {
        &self.draw
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Component)]
pub struct DisableCulling;

pub struct DrawIndex<D: Draw>(usize, std::marker::PhantomData<D>);
impl<D: Draw> DrawIndex<D> {
    pub fn new(index: usize) -> Self {
        Self(index, Default::default())
    }

    pub fn get(&self) -> usize {
        self.0
    }
}

impl<D: Draw> Copy for DrawIndex<D> {}
impl<D: Draw> Clone for DrawIndex<D> {
    fn clone(&self) -> Self {
        Self(self.0.clone(), self.1.clone())
    }
}

impl<D: Draw> AsRef<usize> for DrawIndex<D> {
    fn as_ref(&self) -> &usize {
        &self.0
    }
}

#[derive(Resource)]
pub struct VisibleDraws<D: Draw>(
    HashMap<Entity, Vec<DrawIndex<D>>>,
    std::marker::PhantomData<D>,
);
impl<D: Draw> VisibleDraws<D> {
    pub fn new() -> Self {
        Self(HashMap::new(), Default::default())
    }

    pub(crate) fn queue(
        visible: &mut Self,
        tree: &mut DrawTree<D>,
        views: &ViewDataBuffer<D::View>,
    ) {
        if D::CULL {
            for (entity, view) in views.views() {
                let mut draws = tree.cull(&view.frustum);
                visible
                    .0
                    .insert(*entity, draws.drain(..).map(|d| *d).collect());
            }
        } else {
            for (entity, _) in views.views() {
                visible
                    .0
                    .insert(*entity, (0..tree.nodes.len()).map(DrawIndex::new).collect());
            }
        }
    }
}

#[derive(Resource)]
pub struct DrawPipeline<D: Draw>(PipelineId, std::marker::PhantomData<D>);
impl<D: Draw> std::ops::Deref for DrawPipeline<D> {
    type Target = PipelineId;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<D: Draw> RenderResource for DrawPipeline<D> {
    type Arg = (
        Write<PipelineCache>,
        Read<RenderSurface>,
        Option<Read<ViewDataBuffer<D::View>>>,
        Option<Read<ModelDataBuffer<D::Model>>>,
        Option<Read<BatchedModelDataBuffer<D::Model>>>,
        Option<Write<MaterialLayout<D::Material>>>,
        SCommands,
    );

    fn extract(arg: ecs::ArgItem<Self::Arg>) -> Result<Self, crate::ExtractError<()>> {
        let (cache, surface, views, model_data, batched_model_data, layout, mut commands) = arg;

        let view_layout = match views {
            Some(views) => views.layout(),
            None => return Err(crate::resources::ExtractError::Retry(())),
        };

        let model_layout = if D::BATCH {
            let Some(batched_model_data) = batched_model_data else {
                return Err(crate::resources::ExtractError::Retry(()));
            };

            batched_model_data.0.layout()
        } else if let Some(model_data) = model_data {
            model_data.0.layout()
        } else {
            return Err(crate::resources::ExtractError::Retry(()));
        };

        let material_layout = match layout {
            Some(layout) => layout,
            None => {
                commands.add(ExtractResource::<MaterialLayout<D::Material>>::new());
                return Err(crate::resources::ExtractError::Retry(()));
            }
        };

        let vertex_shader: AssetId<Shader> = D::shader().into();
        let fragment_shader: AssetId<Shader> = D::Material::shader().into();

        assert!(
            !D::formats().is_empty(),
            "Mesh pipeline must have at least one vertex format"
        );

        let buffers = vec![MeshLayout::into_vertex_buffer_layout(
            0,
            D::formats(),
            VertexStepMode::Vertex,
        )];

        let id = cache.queue_render_pipeline(RenderPipelineDesc {
            label: None,
            layout: vec![
                view_layout.clone(),
                model_layout.clone(),
                material_layout.as_ref().clone(),
            ],
            vertex: VertexState {
                shader: vertex_shader.into(),
                entry: "main".into(),
                buffers,
            },
            fragment: Some(FragmentState {
                shader: fragment_shader.into(),
                entry: "main".into(),
                targets: vec![Some(ColorTargetState {
                    format: surface.format(),
                    blend: Some(<D::Material as Material>::Phase::mode().into()),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: D::primitive_state(),
            depth_stencil: Some(wgpu::DepthStencilState {
                format: surface.depth_format(),
                depth_write_enabled: matches!(
                    <D::Material as Material>::depth_write(),
                    DepthWrite::On
                ),
                depth_compare: wgpu::CompareFunction::Less,
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            push_constants: vec![],
        });

        Ok(Self(id, Default::default()))
    }
}

pub struct DrawSystemId<V: View>(usize, std::marker::PhantomData<V>);
impl<V: View> std::ops::Deref for DrawSystemId<V> {
    type Target = usize;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<V: View> Copy for DrawSystemId<V> {}
impl<V: View> Clone for DrawSystemId<V> {
    fn clone(&self) -> Self {
        Self(self.0.clone(), self.1.clone())
    }
}

pub struct DrawId<D: Draw>(usize, std::marker::PhantomData<D>);
impl<D: Draw> std::ops::Deref for DrawId<D> {
    type Target = usize;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<D: Draw> Copy for DrawId<D> {}
impl<D: Draw> Clone for DrawId<D> {
    fn clone(&self) -> Self {
        Self(self.0.clone(), self.1.clone())
    }
}

impl<V: View, D: Draw<View = V>> From<DrawId<D>> for DrawSystemId<V> {
    fn from(value: DrawId<D>) -> Self {
        Self(value.0, Default::default())
    }
}

impl<V: View> From<usize> for DrawSystemId<V> {
    fn from(value: usize) -> Self {
        Self(value, Default::default())
    }
}

unsafe impl<D: Draw> SystemArg for DrawId<D> {
    type Item<'world, 'state> = Self;

    type State = DrawSystemId<D::View>;

    fn init(world: &mut ecs::World, _: &mut ecs::WorldAccess) -> Self::State {
        let mut functions = world
            .remove_resource::<DrawSystems<D::View>>()
            .unwrap_or_default();

        let id = functions.register::<D>(world, &mut ecs::WorldAccess::new());

        world.add_resource(functions);

        id
    }

    unsafe fn get<'world, 'state>(
        state: &'state mut Self::State,
        _: ecs::world::WorldCell<'world>,
        _: &ecs::SystemMeta,
    ) -> Self::Item<'world, 'state> {
        DrawId(state.0, Default::default())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BatchKey {
    pub material: ErasedId,
    pub mesh: AssetId<Mesh>,
    pub sub_mesh: Option<AssetId<SubMesh>>,
}

pub struct DrawCall<V: View> {
    key: BatchKey,
    item: V::Item,
    dynamic_offset: u32,
    instances: Range<u32>,
    bind_group: usize,
    pipeline: PipelineId,
    system: DrawSystemId<V>,
}

#[derive(Resource)]
pub struct ViewDrawCalls<V: View, R: RenderPhase>(
    pub(crate) HashMap<Entity, Vec<DrawCall<V>>>,
    std::marker::PhantomData<R>,
);

impl<V: View, R: RenderPhase> ViewDrawCalls<V, R> {
    pub fn new() -> Self {
        Self(HashMap::new(), Default::default())
    }

    pub fn get(&self, entity: &Entity) -> Option<&[DrawCall<V>]> {
        self.0.get(entity).map(|v| v.as_slice())
    }

    pub(crate) fn queue<D: Draw<View = V>>(
        view_draws: &mut Self,
        draws: &mut DrawTree<D>,
        visible_draws: &VisibleDraws<D>,
        view_buffer: &ViewDataBuffer<D::View>,
        model_data: &mut ModelDataBuffer<D::Model>,
        batched_model_data: &mut BatchedModelDataBuffer<D::Model>,
        pipeline: &DrawPipeline<D>,
        system: DrawId<D>,
    ) where
        D::Material: Material<Phase = R>,
    {
        let system = system.into();

        if D::BATCH {
            for (entity, _) in view_buffer.views() {
                let Some(indicies) = visible_draws.0.get(entity) else {
                    continue;
                };

                let mut batches = HashMap::new();

                for index in indicies {
                    let draw = &draws.nodes[index.0];
                    let key = BatchKey {
                        material: draw.as_ref().material().into(),
                        mesh: draw.as_ref().mesh(),
                        sub_mesh: draw.as_ref().sub_mesh(),
                    };

                    batches.entry(key).or_insert(Vec::new()).push(draw.data());
                }

                let draw_calls = batches
                    .drain()
                    .map(|(key, data)| {
                        let item = V::Item::default();

                        batched_model_data
                            .push(&data)
                            .drain(..)
                            .map(|batch| DrawCall::<V> {
                                key,
                                item,
                                dynamic_offset: 0,
                                instances: batch.instances,
                                bind_group: batch.bind_group,
                                pipeline: **pipeline,
                                system,
                            })
                            .collect::<Vec<_>>()
                    })
                    .flatten();

                view_draws.0.entry(*entity).or_default().extend(draw_calls);
            }
        } else {
            for (entity, view) in view_buffer.views() {
                let Some(indicies) = visible_draws.0.get(entity) else {
                    continue;
                };

                let draw_calls = indicies.iter().map(|index| {
                    let draw = &draws.nodes[index.0];
                    let key = BatchKey {
                        material: draw.as_ref().material().into(),
                        mesh: draw.as_ref().mesh(),
                        sub_mesh: draw.as_ref().sub_mesh(),
                    };

                    let (bind_group, dynamic_offset) = model_data.push(&draw.data());
                    let item = view.item(R::mode(), &draw.local_transform, &draw.global_transform);

                    DrawCall::<V> {
                        key,
                        item,
                        dynamic_offset,
                        instances: 0..1,
                        bind_group,
                        pipeline: **pipeline,
                        system,
                    }
                });

                view_draws.0.entry(*entity).or_default().extend(draw_calls);
            }
        }

        draws.clear();
    }

    pub(crate) fn sort(draws: &mut Self) {
        for calls in draws.0.values_mut() {
            calls.sort_by(|a, b| {
                a.item
                    .partial_cmp(&b.item)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
        }
    }

    pub(crate) fn clear(draws: &mut Self) {
        draws.0.clear();
    }
}

pub enum DrawError {
    Skip,
}

pub trait DrawCommand<V: View> {
    type Arg: ReadOnly;

    fn execute<'w>(
        state: &mut RenderState<'w>,
        view: &RenderView<V>,
        draw: &DrawCall<V>,
        arg: ArgItem<'w, 'w, Self::Arg>,
    ) -> Result<(), DrawError>;
}

pub struct SetPipeline<V: View>(std::marker::PhantomData<V>);
impl<V: View> DrawCommand<V> for SetPipeline<V> {
    type Arg = Option<Read<PipelineCache>>;

    fn execute<'w>(
        state: &mut RenderState<'w>,
        _: &RenderView<V>,
        draw: &DrawCall<V>,
        cache: ArgItem<'w, 'w, Self::Arg>,
    ) -> Result<(), DrawError> {
        let pipeline = cache
            .ok_or(DrawError::Skip)?
            .get_render_pipeline(&draw.pipeline)
            .ok_or(DrawError::Skip)?;

        Ok(state.set_pipeline(pipeline))
    }
}

pub struct SetView<V: View, const GROUP: u32>(std::marker::PhantomData<V>);
impl<V: View, const GROUP: u32> DrawCommand<V> for SetView<V, GROUP> {
    type Arg = Read<ViewDataBuffer<V>>;

    fn execute<'w>(
        state: &mut RenderState<'w>,
        view: &RenderView<V>,
        _: &DrawCall<V>,
        views: ArgItem<'w, 'w, Self::Arg>,
    ) -> Result<(), DrawError> {
        Ok(state.set_bind_group(GROUP, views.bind_group(), &[view.dynamic_offset]))
    }
}

pub struct SetMesh<V: View, M: ModelData, const GROUP: u32, const BATCH: bool>(
    std::marker::PhantomData<(V, M)>,
);
impl<V: View, M: ModelData, const GROUP: u32> DrawCommand<V> for SetMesh<V, M, GROUP, true> {
    type Arg = Read<BatchedModelDataBuffer<M>>;

    fn execute<'w>(
        state: &mut RenderState<'w>,
        _: &RenderView<V>,
        draw: &DrawCall<V>,
        batched_model_data: ArgItem<'w, 'w, Self::Arg>,
    ) -> Result<(), DrawError> {
        let bind_group = &batched_model_data.0.bind_groups()[draw.bind_group];
        Ok(state.set_bind_group(GROUP, bind_group, &[]))
    }
}

impl<V: View, M: ModelData, const GROUP: u32> DrawCommand<V> for SetMesh<V, M, GROUP, false> {
    type Arg = Read<ModelDataBuffer<M>>;

    fn execute<'w>(
        state: &mut RenderState<'w>,
        _: &RenderView<V>,
        draw: &DrawCall<V>,
        model_data: ArgItem<'w, 'w, Self::Arg>,
    ) -> Result<(), DrawError> {
        let bind_group = &model_data.0.bind_groups()[draw.bind_group];
        Ok(state.set_bind_group(GROUP, bind_group, &[draw.dynamic_offset]))
    }
}

pub struct SetMaterial<V: View, M: Material, const GROUP: u32>(std::marker::PhantomData<(V, M)>);
impl<V: View, M: Material, const GROUP: u32> DrawCommand<V> for SetMaterial<V, M, GROUP> {
    type Arg = Read<RenderAssets<MaterialBinding<M>>>;

    fn execute<'w>(
        state: &mut RenderState<'w>,
        _: &RenderView<V>,
        draw: &DrawCall<V>,
        materials: ArgItem<'w, 'w, Self::Arg>,
    ) -> Result<(), DrawError> {
        let material = materials
            .get(&draw.key.material.into())
            .ok_or(DrawError::Skip)?;

        Ok(state.set_bind_group(GROUP, &material, &[]))
    }
}

pub struct DrawMesh<V: View, M: ModelData>(std::marker::PhantomData<(V, M)>);
impl<V: View, M: ModelData> DrawCommand<V> for DrawMesh<V, M> {
    type Arg = (Read<RenderAssets<RenderMesh>>, Read<RenderAssets<SubMesh>>);

    fn execute<'w>(
        state: &mut RenderState<'w>,
        _: &RenderView<V>,
        draw: &DrawCall<V>,
        (meshes, sub_meshes): ArgItem<'w, 'w, Self::Arg>,
    ) -> Result<(), DrawError> {
        let DrawCall { key, instances, .. } = draw;

        let mesh = meshes.get(&(key.mesh).into()).ok_or(DrawError::Skip)?;
        let submesh = key
            .sub_mesh
            .and_then(|id| sub_meshes.get(&id.into()))
            .copied()
            .unwrap_or(SubMesh::from(mesh));

        let vertices = submesh.start_vertex..submesh.start_vertex + submesh.vertex_count;
        let indices = submesh.start_index..submesh.start_index + submesh.index_count;

        state.set_vertex_buffer(0, mesh.vertex_buffer().slice(..));

        match mesh.index_buffer() {
            Some(buffer) => {
                state.set_index_buffer(buffer.slice(..));
                Ok(state.draw_indexed(indices, vertices.start as i32, instances.clone()))
            }
            None => Ok(state.draw(vertices, instances.clone())),
        }
    }
}

#[derive(Resource)]
pub struct DrawSystems<V: View>(IndexMap<TypeId, DrawSystem<V>>);
impl<V: View> Default for DrawSystems<V> {
    fn default() -> Self {
        Self(Default::default())
    }
}

impl<V: View> DrawSystems<V> {
    pub fn register<D: Draw<View = V>>(
        &mut self,
        world: &mut World,
        access: &mut WorldAccess,
    ) -> DrawSystemId<V> {
        let ty = TypeId::of::<D>();
        if let Some(id) = self.0.get_index_of(&ty).map(DrawSystemId::<V>::from) {
            return id;
        };

        let id = DrawSystemId::from(self.0.len());
        let system = DrawSystem::<V>::new::<D>(world, access);

        self.0.insert(ty, system);

        id
    }
}

type DrawFunction<D, const BATCH: bool> = (
    SetPipeline<<D as Draw>::View>,
    SetView<<D as Draw>::View, 0>,
    SetMesh<<D as Draw>::View, <D as Draw>::Model, 1, BATCH>,
    SetMaterial<<D as Draw>::View, <D as Draw>::Material, 2>,
    DrawMesh<<D as Draw>::View, <D as Draw>::Model>,
);

pub struct DrawSystem<V: View> {
    state: SystemState,
    function: for<'w> fn(
        &mut RenderState<'w>,
        &mut SystemState,
        WorldCell<'w>,
        &RenderView<V>,
        &DrawCall<V>,
        &'w SystemMeta,
    ) -> Result<(), DrawError>,
    _marker: std::marker::PhantomData<V>,
}

impl<V: View> DrawSystem<V> {
    fn new<D: Draw<View = V>>(world: &mut World, access: &mut WorldAccess) -> Self {
        let state = if D::BATCH {
            Box::new(<DrawFunction<D, true> as DrawCommand<V>>::Arg::init(
                world, access,
            ))
        } else {
            Box::new(<DrawFunction<D, false> as DrawCommand<V>>::Arg::init(
                world, access,
            ))
        };

        Self {
            state,
            function: if D::BATCH {
                |render, state, world, view, draw, meta| {
                    let state = state.downcast_mut::<<<DrawFunction::<D, true> as DrawCommand<V>>::Arg as SystemArg>::State>().unwrap();
                    let arg = unsafe {
                        <DrawFunction<D, true> as DrawCommand<V>>::Arg::get(state, world, meta)
                    };

                    DrawFunction::<D, true>::execute(render, view, draw, arg)
                }
            } else {
                |render, state, world, view, draw, meta| {
                    let state = state.downcast_mut::<<<DrawFunction::<D, false> as DrawCommand<V>>::Arg as SystemArg>::State>().unwrap();
                    let arg = unsafe {
                        <DrawFunction<D, false> as DrawCommand<V>>::Arg::get(state, world, meta)
                    };

                    DrawFunction::<D, false>::execute(render, view, draw, arg)
                }
            },
            _marker: Default::default(),
        }
    }

    fn draw<'w>(
        &mut self,
        state: &mut RenderState<'w>,
        world: WorldCell<'w>,
        view: &RenderView<V>,
        draw: &DrawCall<V>,
        meta: &'w SystemMeta,
    ) -> Result<(), DrawError> {
        (self.function)(state, &mut self.state, world, view, draw, meta)
    }
}

pub struct RenderPhases(
    Vec<(
        for<'a> fn(Entity, &RenderContext<'a>, &mut RenderState<'a>, &'a SystemMeta),
        usize,
    )>,
);

impl RenderPhases {
    pub fn add_phase<V: View, P: RenderPhase>(&mut self) {
        self.0.push((
            |entity, ctx, state, meta| {
                let mut world = unsafe { ctx.world().cell() };
                let systems = unsafe { world.get_mut().resource_mut::<DrawSystems<V>>() };
                let views = unsafe { world.get().resource::<ViewDataBuffer<V>>() };
                let draw_calls = unsafe { world.get().resource::<ViewDrawCalls<V, P>>() };

                let Some(calls) = draw_calls.get(&entity) else {
                    return;
                };

                let Some(view) = views.get(&entity) else {
                    return;
                };

                for draw in calls {
                    let system = &mut systems.0[*draw.system];

                    let _ = system.draw(state, world, view, draw, meta);
                }
            },
            P::mode() as usize,
        ));
    }

    fn render<'a>(
        &self,
        entity: Entity,
        ctx: &RenderContext<'a>,
        mut state: RenderState<'a>,
        meta: &'a SystemMeta,
    ) {
        for phase in &self.0 {
            phase.0(entity, ctx, &mut state, meta);
        }
    }
}

pub trait Renderer: Send + Sync + 'static {
    const NAME: super::Name;

    type Data: Send + Sync + 'static;

    fn setup(builder: &mut PassBuilder, phases: &mut RenderPhases) -> Self::Data;

    fn attachments<'a>(
        _ctx: &'a RenderContext<'a>,
        _data: &Self::Data,
    ) -> Vec<Option<wgpu::RenderPassColorAttachment<'a>>> {
        vec![]
    }
}

pub struct RendererPass<R: Renderer>(std::marker::PhantomData<R>);
impl<R: Renderer> Default for RendererPass<R> {
    fn default() -> Self {
        Self(Default::default())
    }
}

impl<R: Renderer> RenderGraphPass for RendererPass<R> {
    const NAME: super::Name = R::NAME;

    fn setup(
        self,
        builder: &mut PassBuilder,
    ) -> impl Fn(&mut RenderContext) + Send + Sync + 'static {
        let mut phases = RenderPhases(Vec::new());
        let data = R::setup(builder, &mut phases);
        let color = builder.write::<RenderOutput>();
        let depth = builder.create::<DepthOutput>(());

        phases.0.sort_by_key(|p| p.1);

        move |ctx| {
            let Some(camera) = ctx.camera() else {
                return;
            };

            let color = ctx.get::<RenderOutput>(color);
            let depth = ctx.get::<DepthOutput>(depth);

            let mut encoder = ctx.encoder();
            let mut color_attachments = vec![Some(wgpu::RenderPassColorAttachment {
                view: color,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(camera.clear_color.unwrap_or(Color::black()).into()),
                    store: wgpu::StoreOp::Store,
                },
            })];

            color_attachments.extend(R::attachments(ctx, &data));
            let depth_stencil_attachment = wgpu::RenderPassDepthStencilAttachment {
                view: depth,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0f32),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            };

            let desc = wgpu::RenderPassDescriptor {
                label: Some(R::NAME),
                color_attachments: &color_attachments,
                depth_stencil_attachment: Some(depth_stencil_attachment),
                timestamp_writes: Default::default(),
                occlusion_query_set: Default::default(),
            };

            let state = RenderState::new(encoder.begin_render_pass(&desc));
            phases.render(camera.entity, ctx, state, ctx.meta());

            ctx.submit(encoder.finish());
        }
    }
}

#[macro_export]
macro_rules! impl_draw_command_for_tuples {
    ($($name:ident),*) => {
            #[allow(non_snake_case)]
            impl<V: View, $($name: DrawCommand<V>),+> DrawCommand<V> for ($($name),*) {
                type Arg = ($($name::Arg,)*);

                fn execute<'w>(
                    state: &mut RenderState<'w>,
                    view: &RenderView<V>,
                    draw: &DrawCall<V>,
                    arg: ArgItem<'w, 'w, Self::Arg>,
                ) -> Result<(), DrawError> {
                    let ($($name,)*) = arg;
                    $(
                        $name::execute(state, view, draw, $name)?;
                    )*

                    Ok(())
                }
            }
    };
}

impl_draw_command_for_tuples!(A, B);
impl_draw_command_for_tuples!(A, B, C);
impl_draw_command_for_tuples!(A, B, C, D);
impl_draw_command_for_tuples!(A, B, C, D, E);
impl_draw_command_for_tuples!(A, B, C, D, E, F2);
impl_draw_command_for_tuples!(A, B, C, D, E, F2, G);
impl_draw_command_for_tuples!(A, B, C, D, E, F2, G, H);
impl_draw_command_for_tuples!(A, B, C, D, E, F2, G, H, I);
impl_draw_command_for_tuples!(A, B, C, D, E, F2, G, H, I, J);
