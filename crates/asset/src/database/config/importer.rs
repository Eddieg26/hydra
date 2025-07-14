use crate::{
    asset::{Asset, AssetDependencies, AssetId, AssetType, ErasedId},
    database::{
        config::{
            cache::{
                Artifact, ArtifactPath, AssetCache, AssetMetadata, Checksum, ImportDependency,
                LoadedAsset,
            },
            registry::AssetRegistry,
            sources::AssetFileSystem,
        },
        library::AssetLibrary,
    },
    ext::{DeserializeExt, SerializeExt},
    io::{AsyncIoError, AsyncReader, BoxFuture, LoadPath, path::AssetPath},
    settings::{AssetSettings, ErasedAssetSettings, Settings},
};
use serde::{Deserialize, Serialize};
use std::{any::TypeId, collections::HashMap, error::Error, marker::PhantomData, path::Path};

#[derive(Debug, thiserror::Error)]
pub enum AddChildAssetError {
    #[error("Failed to add child asset: {name}")]
    UnregisteredType { name: String },
    #[error("Duplicate child asset: {name}")]
    DuplicateChild { name: String },
    #[error("Failed to serialize child asset '{name}': {error}")]
    Serialize {
        name: String,
        error: bincode::error::EncodeError,
    },
}

pub struct ImportContext<'a> {
    fs: &'a AssetFileSystem,
    registry: &'a AssetRegistry,
    path: &'a AssetPath<'a>,
    id: ErasedId,
    ty: AssetType,
    processor: Option<AssetProcessorId>,
    children: HashMap<ErasedId, Artifact>,
}

impl<'a> ImportContext<'a> {
    pub fn new(
        fs: &'a AssetFileSystem,
        registry: &'a AssetRegistry,
        path: &'a AssetPath<'a>,
        id: ErasedId,
        ty: AssetType,
    ) -> Self {
        Self {
            fs,
            registry,
            path,
            id,
            ty,
            processor: None,
            children: HashMap::new(),
        }
    }

    pub fn path(&self) -> &AssetPath<'a> {
        self.path
    }

    pub fn ty(&self) -> AssetType {
        self.ty
    }

    pub fn set_processor(&mut self, processor: AssetProcessorId) {
        self.processor = Some(processor);
    }

    pub fn add_child<A: Asset + Serialize + for<'de> Deserialize<'de>>(
        &mut self,
        name: &str,
        asset: A,
    ) -> Result<AssetId<A>, AddChildAssetError> {
        let ty = TypeId::of::<A>();
        let ty = match self.registry.get_ty(ty) {
            Some(ty) => ty,
            None => {
                return Err(AddChildAssetError::UnregisteredType {
                    name: name.to_owned(),
                });
            }
        };

        let id = self.id.with_namespace(name.as_bytes());
        if self.children.contains_key(&id) {
            return Err(AddChildAssetError::DuplicateChild {
                name: name.to_owned(),
            });
        }

        let path = self.path.to_owned().with_name(name.to_owned());
        let metadata = AssetMetadata::new(id, ty, path).with_parent(self.id);
        let data = asset
            .to_bytes()
            .map_err(|error| AddChildAssetError::Serialize {
                name: name.to_owned(),
                error,
            })?;

        let artifact = unsafe { Artifact::from_raw_parts(data, metadata) };
        self.children.insert(id, artifact);

        Ok(id.into())
    }
}

pub trait AssetImporter: 'static {
    type Asset: Asset + SerializeExt + DeserializeExt;

    type Settings: Settings;

    type Error: Error + Send + Sync + 'static;

    fn import(
        ctx: &mut ImportContext,
        reader: &mut dyn AsyncReader,
        settings: &AssetSettings<Self::Settings>,
    ) -> impl Future<Output = Result<Self::Asset, Self::Error>> + Send;

    fn extensions() -> &'static [&'static str];
}

pub struct ProcessContext<'a> {
    library: &'a AssetLibrary,
    cache: &'a AssetCache,
    path: &'a AssetPath<'a>,
    ty: AssetType,
    pub(crate) dependencies: Vec<ImportDependency>,
}

impl<'a> ProcessContext<'a> {
    pub fn new(
        library: &'a AssetLibrary,
        cache: &'a AssetCache,
        path: &'a AssetPath<'a>,
        ty: AssetType,
    ) -> Self {
        Self {
            library,
            cache,
            path,
            ty,
            dependencies: Vec::new(),
        }
    }

    pub fn path(&self) -> &Path {
        self.path
    }

    pub fn ty(&self) -> AssetType {
        self.ty
    }

    pub async fn load<A: Asset + for<'de> Deserialize<'de>>(
        &mut self,
        path: impl Into<LoadPath<'static>>,
    ) -> Result<LoadedAsset<A>, AsyncIoError> {
        let path: LoadPath<'static> = path.into();
        let id = match path {
            LoadPath::Path(path) => *self
                .library
                .get(&path)
                .ok_or(AsyncIoError::NotFound((&path).into()))?,
            LoadPath::Id(id) => id,
        };

        let artifact = self.cache.read_artifact(ArtifactPath::Cache, &id).await?;

        let asset = A::from_bytes(artifact.data()).map_err(AsyncIoError::from)?;
        self.dependencies.push(ImportDependency::new(
            id.into(),
            artifact.meta.import.full_checksum,
        ));

        Ok(LoadedAsset::new(asset, artifact.meta))
    }
}

pub trait AssetProcessor: 'static {
    type Input: Asset + SerializeExt + DeserializeExt;

    type Output: Asset + SerializeExt + DeserializeExt;

    type Error: Error + Send + Sync + 'static;

    fn process(
        ctx: &mut ProcessContext,
        asset: Self::Input,
    ) -> impl Future<Output = Result<Self::Output, Self::Error>> + Send;
}

pub struct DefaultProcessor<I: AssetImporter>(PhantomData<I>);
impl<I: AssetImporter> AssetProcessor for DefaultProcessor<I> {
    type Input = I::Asset;
    type Output = I::Asset;
    type Error = I::Error;

    async fn process(
        _: &mut ProcessContext<'_>,
        asset: Self::Input,
    ) -> Result<Self::Output, Self::Error> {
        Ok(asset)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ErasedImporter {
    import: for<'a> fn(
        ImportContext<'a>,
        &'a mut dyn AsyncReader,
        &'a dyn ErasedAssetSettings,
    ) -> BoxFuture<'a, Result<Vec<Artifact>, Box<dyn Error + Send + Sync>>>,
    deserialize_settings: fn(&[u8]) -> Result<Box<dyn ErasedAssetSettings>, AsyncIoError>,
    default_settings: fn() -> Box<dyn ErasedAssetSettings>,
    type_id: fn() -> TypeId,
    asset_type_id: fn() -> TypeId,
    extensions: fn() -> &'static [&'static str],
}

impl ErasedImporter {
    pub fn new<I: AssetImporter>() -> Self {
        Self {
            import: |mut ctx, reader, settings| {
                let f = async move {
                    let settings = settings
                        .downcast_ref::<AssetSettings<I::Settings>>()
                        .ok_or(
                            Box::new(AsyncIoError::invalid_data()) as Box<dyn Error + Send + Sync>
                        )?;

                    let asset = I::import(&mut ctx, reader, settings)
                        .await
                        .map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)?;

                    let checksum = {
                        let asset = ctx
                            .fs
                            .read(&ctx.path)
                            .await
                            .map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)?;
                        let settings = ctx
                            .fs
                            .save_settings(ctx.path(), settings)
                            .await
                            .map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)?;

                        Checksum::from(&asset, &settings)
                    };

                    let data = asset.to_bytes().map_err(|e| {
                        Box::new(AsyncIoError::from(e)) as Box<dyn Error + Send + Sync>
                    })?;

                    let mut dependencies = Vec::new();
                    asset.get_dependencies(|id| dependencies.push(id));

                    let children = ctx.children.keys().cloned().collect();
                    let metadata = AssetMetadata::new(settings.id, ctx.ty, ctx.path.to_owned())
                        .with_processor(ctx.processor)
                        .with_checksum(checksum)
                        .with_children(children)
                        .with_dependencies(dependencies);

                    let artifact = unsafe { Artifact::from_raw_parts(data, metadata) };
                    let mut artifacts = vec![artifact];
                    artifacts.extend(ctx.children.into_values());

                    Ok(artifacts)
                };

                Box::pin(f)
            },
            deserialize_settings: |data| {
                AssetSettings::<I::Settings>::from_bytes(data)
                    .map(|metadata| Box::new(metadata) as Box<dyn ErasedAssetSettings>)
                    .map_err(AsyncIoError::from)
            },
            default_settings: || Box::new(AssetSettings::<I::Settings>::default()),
            type_id: || TypeId::of::<I>(),
            asset_type_id: || TypeId::of::<I::Asset>(),
            extensions: <I as AssetImporter>::extensions,
        }
    }

    pub fn import<'a>(
        &'a self,
        ctx: ImportContext<'a>,
        reader: &'a mut dyn AsyncReader,
        settings: &'a dyn ErasedAssetSettings,
    ) -> BoxFuture<'a, Result<Vec<Artifact>, Box<dyn Error + Send + Sync>>> {
        (self.import)(ctx, reader, settings)
    }

    pub fn deserialize_settings(
        &self,
        data: &[u8],
    ) -> Result<Box<dyn ErasedAssetSettings>, AsyncIoError> {
        match (self.deserialize_settings)(data) {
            Ok(settings) => Ok(settings),
            Err(error) => Err(error),
        }
    }

    pub fn default_settings(&self) -> Box<dyn ErasedAssetSettings> {
        (self.default_settings)()
    }

    pub fn asset_type(&self) -> TypeId {
        (self.asset_type_id)()
    }

    pub fn type_id(&self) -> TypeId {
        (self.type_id)()
    }

    pub fn extensions(&self) -> &'static [&'static str] {
        (self.extensions)()
    }
}

pub struct ErasedProcesser {
    process: for<'a> fn(
        &'a mut ProcessContext,
        &'a [u8],
    ) -> BoxFuture<'a, Result<Vec<u8>, Box<dyn Error + Send + Sync>>>,
    output_asset: fn() -> TypeId,
}

impl ErasedProcesser {
    pub fn new<P: AssetProcessor>() -> Self {
        Self {
            process: |ctx, data| {
                let f = async move {
                    let input = P::Input::from_bytes(&data)
                        .map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)?;
                    let output = P::process(ctx, input)
                        .await
                        .map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)?;

                    output
                        .to_bytes()
                        .map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)
                };

                Box::pin(f)
            },
            output_asset: || TypeId::of::<P::Output>(),
        }
    }

    pub fn process<'a>(
        &'a self,
        ctx: &'a mut ProcessContext,
        data: &'a [u8],
    ) -> BoxFuture<'a, Result<Vec<u8>, Box<dyn Error + Send + Sync>>> {
        (self.process)(ctx, data)
    }

    pub fn output_asset(&self) -> TypeId {
        (self.output_asset)()
    }
}

pub struct AssetImporters {
    importers: Vec<ErasedImporter>,
    ty_map: HashMap<TypeId, usize>,
    ext_map: HashMap<&'static str, usize>,
}

impl AssetImporters {
    pub fn new() -> Self {
        Self {
            importers: Vec::new(),
            ty_map: HashMap::new(),
            ext_map: HashMap::new(),
        }
    }

    pub fn get(&self, ty: TypeId) -> Option<&ErasedImporter> {
        self.ty_map.get(&ty).map(|i| &self.importers[*i])
    }

    pub fn get_by_ext(&self, ext: &str) -> Option<&ErasedImporter> {
        self.ext_map.get(ext).map(|index| &self.importers[*index])
    }

    pub fn add<I: AssetImporter>(&mut self) {
        let ty = TypeId::of::<I>();
        if !self.ty_map.contains_key(&ty) {
            let index = self.importers.len();
            self.importers.push(ErasedImporter::new::<I>());
            self.ty_map.entry(ty).or_insert(index);
            for ext in I::extensions() {
                self.ext_map.insert(ext, index);
            }
        }
    }

    pub fn contains(&self, ty: &TypeId) -> bool {
        self.ty_map.contains_key(ty)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AssetProcessorId(u32);

pub struct AssetProcessors {
    processors: Vec<ErasedProcesser>,
    ty_map: HashMap<TypeId, usize>,
    defaults: HashMap<AssetType, usize>,
}

impl AssetProcessors {
    pub fn new() -> Self {
        Self {
            processors: Vec::new(),
            ty_map: HashMap::new(),
            defaults: HashMap::new(),
        }
    }

    pub fn get(&self, id: AssetProcessorId) -> &ErasedProcesser {
        &self.processors[id.0 as usize]
    }

    pub fn get_default(&self, ty: AssetType) -> Option<&ErasedProcesser> {
        let index = self.defaults.get(&ty).copied()?;
        Some(&self.processors[index])
    }

    pub fn add<P: AssetProcessor>(&mut self) -> AssetProcessorId {
        let ty = TypeId::of::<P>();
        if let Some(id) = self.ty_map.get(&ty).map(|i| AssetProcessorId(*i as u32)) {
            id
        } else {
            let id = AssetProcessorId(self.processors.len() as u32);
            let processor = ErasedProcesser::new::<P>();
            self.processors.push(processor);
            self.ty_map.insert(ty, id.0 as usize);
            id
        }
    }

    pub fn set_default<P: AssetProcessor>(&mut self, ty: AssetType) -> AssetProcessorId {
        let id = self.add::<P>();
        self.defaults.insert(ty, id.0 as usize);

        id
    }
}
