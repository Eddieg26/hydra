use asset::{AssetEvent, AssetId, Assets, asset::Asset};
use ecs::{
    Command, Commands, Event, EventReader, EventWriter, IndexMap, IntoSystemConfig, Resource,
    SystemId, SystemMeta, WorldAccess,
    app::Main,
    system::{ArgItem, SystemArg, SystemConfig, unlifetime::StaticArg},
};
use std::{
    any::TypeId,
    collections::{HashMap, HashSet},
    hash::Hash,
    sync::Arc,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AssetUsage {
    Keep,
    Discard,
}

#[derive(
    Default, Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize,
)]
pub enum ReadWrite {
    Enabled,
    #[default]
    Disabled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RenderAssetType(TypeId);
impl RenderAssetType {
    pub fn of<T: RenderAsset>() -> Self {
        Self(TypeId::of::<T>())
    }
}

#[allow(unused_variables)]
pub trait RenderAsset: Sized + Send + Sync + 'static {
    type Source: Asset + Clone;

    type Arg: SystemArg;

    fn extract(
        asset: Self::Source,
        arg: &mut ArgItem<Self::Arg>,
    ) -> Result<Self, ExtractError<Self::Source>>;

    fn removed(id: &AssetId<Self::Source>, asset: &Self, arg: &mut ArgItem<Self::Arg>) {}

    fn usage(asset: &Self::Source) -> AssetUsage {
        AssetUsage::Discard
    }

    fn dependencies() -> Vec<RenderAssetType> {
        Vec::new()
    }
}

#[derive(Resource)]
pub struct RenderAssets<R: RenderAsset> {
    assets: HashMap<AssetId<R::Source>, R>,
}

impl<R: RenderAsset> RenderAssets<R> {
    pub fn new() -> Self {
        Self {
            assets: HashMap::new(),
        }
    }

    pub fn add(&mut self, id: AssetId<R::Source>, asset: R) {
        self.assets.insert(id, asset);
    }

    pub fn get(&self, id: &AssetId<R::Source>) -> Option<&R> {
        self.assets.get(id)
    }

    pub fn get_mut(&mut self, id: &AssetId<R::Source>) -> Option<&mut R> {
        self.assets.get_mut(id)
    }

    pub fn remove(&mut self, id: &AssetId<R::Source>) -> Option<R> {
        self.assets.remove(id)
    }

    pub fn contains(&self, id: &AssetId<R::Source>) -> bool {
        self.assets.contains_key(id)
    }

    pub fn iter(&self) -> std::collections::hash_map::Iter<'_, AssetId<R::Source>, R> {
        self.assets.iter()
    }

    pub fn iter_mut(&mut self) -> std::collections::hash_map::IterMut<'_, AssetId<R::Source>, R> {
        self.assets.iter_mut()
    }

    pub fn len(&self) -> usize {
        self.assets.len()
    }

    pub fn is_empty(&self) -> bool {
        self.assets.is_empty()
    }

    pub fn clear(&mut self) {
        self.assets.clear();
    }

    pub fn retain<F>(&mut self, f: F)
    where
        F: FnMut(&AssetId<R::Source>, &mut R) -> bool,
    {
        self.assets.retain(f);
    }

    pub fn drain(&mut self) -> std::collections::hash_map::Drain<'_, AssetId<R::Source>, R> {
        self.assets.drain()
    }
}

impl<'a, R: RenderAsset> IntoIterator for &'a RenderAssets<R> {
    type Item = (&'a AssetId<R::Source>, &'a R);
    type IntoIter = std::collections::hash_map::Iter<'a, AssetId<R::Source>, R>;

    fn into_iter(self) -> Self::IntoIter {
        self.assets.iter()
    }
}

impl<'a, R: RenderAsset> IntoIterator for &'a mut RenderAssets<R> {
    type Item = (&'a AssetId<R::Source>, &'a mut R);
    type IntoIter = std::collections::hash_map::IterMut<'a, AssetId<R::Source>, R>;

    fn into_iter(self) -> Self::IntoIter {
        self.assets.iter_mut()
    }
}

#[derive(Resource)]
pub struct ExtractInfo<R: RenderAsset> {
    pub extracted: Vec<(AssetId<R::Source>, R::Source)>,
    pub removed: HashSet<AssetId<R::Source>>,
}

impl<R: RenderAsset> ExtractInfo<R> {
    pub fn new() -> Self {
        Self {
            extracted: Vec::new(),
            removed: HashSet::new(),
        }
    }
}

pub struct AssetExtractorConfig {
    pub ty: RenderAssetType,
    pub extract: SystemConfig,
    pub process: SystemConfig,
}

#[derive(Default, Resource)]
pub struct AssetExtractors {
    registry: IndexMap<TypeId, AssetExtractorConfig>,
    dependencies: HashMap<RenderAssetType, Vec<SystemId>>,
}

impl AssetExtractors {
    pub fn add<R: RenderAsset>(&mut self) -> bool {
        let ty = TypeId::of::<R>();
        if self.registry.contains_key(&ty) {
            return false;
        }

        let render_asset_type = RenderAssetType::of::<R>();

        let config = AssetExtractorConfig {
            ty: render_asset_type,
            extract: Self::extractor::<R>.config(),
            process: Self::process::<R>.config(),
        };

        for dep in R::dependencies() {
            self.dependencies
                .entry(dep)
                .or_default()
                .push(config.process.id());
        }

        self.registry.insert(ty, config);

        true
    }

    fn extractor<R: RenderAsset>(
        mut assets: Main<&mut Assets<R::Source>>,
        extract_info: &mut ExtractInfo<R>,
        events: Main<EventReader<AssetEvent<R::Source>>>,
    ) {
        extract_info.removed.clear();

        for event in events.into_inner() {
            match event {
                AssetEvent::Added { id }
                | AssetEvent::Modified { id }
                | AssetEvent::Loaded { id } => {
                    let asset = match assets.get_mut(id).cloned() {
                        Some(source) => source,
                        None => continue,
                    };

                    if R::usage(&asset) == AssetUsage::Discard {
                        assets.remove(id);
                    }

                    extract_info.extracted.push((*id, asset));
                }
                AssetEvent::Removed { id, .. } => {
                    extract_info.removed.insert(*id);
                }
            }
        }
    }

    fn process<R: RenderAsset>(
        mut errors: EventWriter<ExtractError<R>>,
        extract_info: &mut ExtractInfo<R>,
        assets: &mut RenderAssets<R>,
        arg: StaticArg<R::Arg>,
    ) {
        let mut arg = arg.into_inner();
        let extract = std::mem::take(&mut extract_info.extracted);

        for (id, asset) in extract {
            match R::extract(asset, &mut arg) {
                Ok(render_asset) => {
                    assets.add(id.into(), render_asset);
                }
                Err(ExtractError::Retry(asset)) => {
                    extract_info.extracted.push((id, asset));
                }
                Err(ExtractError::Error(error)) => {
                    errors.send(ExtractError::Error(error));
                }
            }
        }

        assets.retain(|id, _| !extract_info.removed.contains(&id));
    }

    pub fn build(self) -> Vec<AssetExtractorConfig> {
        let Self {
            registry,
            dependencies,
        } = self;

        registry
            .into_values()
            .map(|mut config| {
                let Some(dependencies) = dependencies.get(&config.ty) else {
                    return config;
                };

                for dep in dependencies {
                    config.process.add_dependency(*dep);
                }

                config
            })
            .collect()
    }
}

pub trait RenderResource: Resource + Send + Sync + Sized {
    type Arg: SystemArg;

    fn extract(arg: ArgItem<Self::Arg>) -> Result<Self, ExtractError<()>>;
}

pub struct ExtractResource<R: RenderResource>(std::marker::PhantomData<R>);
impl<R: RenderResource> ExtractResource<R> {
    pub fn new() -> Self {
        Self(std::marker::PhantomData)
    }
}

impl<R: RenderResource> Command for ExtractResource<R> {
    fn execute(self, world: &mut ecs::World) {
        world.resource_mut::<ResourceExtractors>().add::<R>();
    }
}

#[derive(Default, Resource)]
pub struct ResourceExtractors(HashMap<TypeId, fn(&mut ecs::World)>);

impl ResourceExtractors {
    pub fn add<R: RenderResource>(&mut self) {
        self.0
            .entry(TypeId::of::<R>())
            .or_insert_with(|| Self::extract_resource::<R>);
    }

    pub(crate) fn extract(
        main_extractors: Main<&mut ResourceExtractors>,
        extractors: &mut ResourceExtractors,
    ) {
        
        extractors.0.extend(main_extractors.into_inner().0.drain());
    }

    pub(crate) fn process(extractors: &mut ResourceExtractors, mut commands: Commands) {
        extractors
            .0
            .drain()
            .for_each(|(_, command)| commands.add(command));
    }

    fn extract_resource<R: RenderResource>(world: &mut ecs::World) {
        let mut state = R::Arg::init(world, &mut WorldAccess::new());
        let meta = SystemMeta::default();
        let arg = unsafe { R::Arg::get(&mut state, world.cell(), &meta) };

        match R::extract(arg) {
            Ok(resource) => {
                world.add_resource(resource);
            }
            Err(ExtractError::Retry(_)) => world.resource_mut::<Self>().add::<R>(),
            Err(ExtractError::Error(error)) => world.send(ExtractError::<R>::Error(error)),
        }

        R::Arg::update(&mut state, world);
    }
}

#[derive(Debug, Clone)]
pub enum ExtractError<T: Send + Sync + 'static = ()> {
    Retry(T),
    Error(Arc<dyn std::error::Error + Send + Sync + 'static>),
}

impl<T: Send + Sync + 'static> ExtractError<T> {
    pub fn from_error<E: std::error::Error + Send + Sync + 'static>(error: E) -> Self {
        Self::Error(Arc::new(error))
    }
}

impl<T: Send + Sync + 'static> Event for ExtractError<T> {}
