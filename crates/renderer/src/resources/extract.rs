use asset::{Asset, AssetEvent, AssetId, Assets};
use ecs::{
    ArgItem, Event, EventReader, EventWriter, FixedBitSet, IntoSystemConfig, Plugin, Resource,
    SystemArg, SystemConfig, system::Main, unlifetime::StaticArg,
};
use std::{any::TypeId, collections::HashMap, error::Error, marker::PhantomData, sync::Arc};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AssetUsage {
    Keep,
    Discard,
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReadWrite {
    Enabled,
    #[default]
    Disabled,
}

#[derive(Debug, Clone)]
pub enum ExtractError<T: Send + Sync + 'static> {
    Retry(T),
    Error(Arc<dyn Error + Send + Sync + 'static>),
}

impl<T: Send + Sync + 'static> ExtractError<T> {
    pub fn from_error<E: Error + Send + Sync + 'static>(error: E) -> Self {
        Self::Error(Arc::from(error))
    }
}

impl<T: Send + Sync + 'static> From<T> for ExtractError<T> {
    fn from(value: T) -> Self {
        Self::Retry(value)
    }
}

impl<T: Send + Sync + 'static> Event for ExtractError<T> {}

pub trait RenderAssetRegister {
    fn register<R: RenderAsset>(&mut self);
}

#[allow(unused_variables)]
pub trait RenderAsset: Send + Sync + Sized + 'static {
    type Asset: Asset + Clone;

    type Arg: SystemArg;

    fn extract(
        id: AssetId<Self::Asset>,
        asset: Self::Asset,
        arg: &mut ArgItem<Self::Arg>,
    ) -> Result<Self, ExtractError<Self::Asset>>;

    fn removed(id: &AssetId<Self::Asset>, asset: &Self, arg: &mut ArgItem<Self::Arg>);

    fn usage(asset: &Self::Asset) -> AssetUsage {
        AssetUsage::Discard
    }

    fn dependencies(register: impl RenderAssetRegister) {}
}

#[derive(Resource)]
pub struct RenderAssets<R: RenderAsset>(HashMap<AssetId<R::Asset>, R>);
impl<R: RenderAsset> Default for RenderAssets<R> {
    fn default() -> Self {
        Self(Default::default())
    }
}

impl<R: RenderAsset> std::ops::Deref for RenderAssets<R> {
    type Target = HashMap<AssetId<R::Asset>, R>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<R: RenderAsset> std::ops::DerefMut for RenderAssets<R> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

pub struct ExtractedAsset<A: Asset + Clone> {
    pub id: AssetId<A>,
    pub asset: A,
}

#[derive(Default, Resource)]
pub struct ExtractInfo<R: RenderAsset> {
    pub assets: Vec<ExtractedAsset<R::Asset>>,
    pub removed: Vec<AssetId<R::Asset>>,
}

pub struct RenderAssetConfig {
    pub extract: SystemConfig,
    pub process: SystemConfig,
    dependencies: FixedBitSet,
}

impl RenderAssetConfig {
    pub fn new<R: RenderAsset>() -> Self {
        Self {
            extract: Self::extract::<R>.config(),
            process: Self::process::<R>.config(),
            dependencies: FixedBitSet::new(),
        }
    }

    fn extract<R: RenderAsset>(
        mut assets: Main<&mut Assets<R::Asset>>,
        extract_info: &mut ExtractInfo<R>,
        events: Main<EventReader<AssetEvent<R::Asset>>>,
    ) {
        for event in events.into_inner() {
            match event {
                AssetEvent::Added { id }
                | AssetEvent::Loaded { id }
                | AssetEvent::Modified { id } => {
                    let Some(asset) = assets.get(id).cloned() else {
                        continue;
                    };

                    if R::usage(&asset) == AssetUsage::Discard {
                        assets.remove(id);
                    }

                    let extracted = ExtractedAsset { id: *id, asset };
                    extract_info.assets.push(extracted);
                }
                AssetEvent::Removed { id, .. } => extract_info.removed.push(*id),
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
        let extract = std::mem::take(&mut extract_info.assets);

        for extracted in extract {
            let ExtractedAsset { id, asset } = extracted;

            match R::extract(id, asset, &mut arg) {
                Ok(render_asset) => {
                    assets.insert(id.into(), render_asset);
                }
                Err(ExtractError::Retry(asset)) => {
                    let extracted = ExtractedAsset { id, asset };
                    extract_info.assets.push(extracted);
                }
                Err(ExtractError::Error(error)) => {
                    errors.send(ExtractError::Error(error));
                }
            }
        }

        assets.retain(|id, _| !extract_info.removed.contains(&id));
        extract_info.removed.clear();
    }
}

#[derive(Resource)]
pub struct RenderAssetRegistry {
    configs: Vec<RenderAssetConfig>,
    indexes: HashMap<TypeId, usize>,
}

impl RenderAssetRegistry {
    pub fn new() -> Self {
        Self {
            configs: Vec::new(),
            indexes: HashMap::new(),
        }
    }

    pub fn register<R: RenderAsset>(&mut self) -> usize {
        let ty = TypeId::of::<R>();
        if let Some(index) = self.indexes.get(&ty) {
            return *index;
        }

        let index = self.configs.len();
        self.indexes.insert(ty, self.configs.len());
        self.configs.push(RenderAssetConfig::new::<R>());

        index
    }

    pub fn add_dependency<R: RenderAsset, Dep: RenderAsset>(&mut self) {
        let asset = self.register::<R>();
        let dependency = self.register::<Dep>();

        self.configs[asset].dependencies.grow(dependency + 1);
        self.configs[asset].dependencies.set(dependency, true);
    }

    pub fn build(self) -> Vec<RenderAssetConfig> {
        let RenderAssetRegistry { mut configs, .. } = self;

        for index in 0..configs.len() {
            for dependency in configs[index].dependencies.clone().ones() {
                let id = configs[dependency].process.id();
                configs[index].process.add_dependency(id);
            }
        }

        configs
    }
}

impl RenderAssetRegister for &mut RenderAssetRegistry {
    fn register<R: RenderAsset>(&mut self) {
        RenderAssetRegistry::register::<R>(self);
    }
}

pub struct RenderAssetPlugin<R: RenderAsset>(PhantomData<R>);

impl<R: RenderAsset> Plugin for RenderAssetPlugin<R> {
    fn setup(&mut self, app: &mut ecs::AppBuilder) {
        app.add_resource(RenderAssets::<R>::default());

        let registry =
            app.get_or_insert_resource::<RenderAssetRegistry>(|| RenderAssetRegistry::new());

        registry.register::<R>();
        R::dependencies(registry);
    }
}
