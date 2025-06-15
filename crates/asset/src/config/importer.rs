use super::{BoxedError, registry::AssetRegistry};
use crate::{
    AssetDependency,
    asset::{Asset, AssetId, AssetMetadata, AssetType, ErasedId, Settings},
    io::{
        Artifact, ArtifactMeta, AssetIoError, AssetPath, AssetSource, AsyncReader, ImportMeta,
        PathExt, SourceName, deserialize,
    },
};
use ecs::Event;
use futures::future::BoxFuture;
use serde::{Deserialize, Serialize};
use std::{
    any::TypeId,
    collections::HashMap,
    path::{Path, PathBuf},
};

pub struct ImportContext<'a> {
    id: ErasedId,
    ty: AssetType,
    source: &'a AssetSource,
    registry: &'a AssetRegistry,
    path: &'a AssetPath<'a>,
    metadata_path: PathBuf,
    processor: Option<u32>,
    children: Vec<Artifact>,
}

impl<'a> ImportContext<'a> {
    pub fn new(
        id: ErasedId,
        ty: AssetType,
        path: &'a AssetPath<'a>,
        source: &'a AssetSource,
        registry: &'a AssetRegistry,
    ) -> Self {
        let metadata_path = path.path().append_ext("meta");

        Self {
            id,
            ty,
            path,
            source,
            registry,
            metadata_path,
            processor: None,
            children: vec![],
        }
    }

    pub fn id(&self) -> ErasedId {
        self.id
    }

    pub fn ty(&self) -> AssetType {
        self.ty
    }

    pub fn path(&self) -> &AssetPath {
        &self.path
    }

    pub fn metadata_path(&self) -> &Path {
        &self.metadata_path
    }

    pub fn set_processor(&mut self, processor: u32) {
        self.processor = Some(processor)
    }

    pub fn add_child<A: Asset + Serialize>(
        &mut self,
        name: impl ToString,
        child: A,
    ) -> Result<AssetId<A>, AssetIoError> {
        let asset_meta = self
            .registry
            .get_ty(TypeId::of::<A>())
            .and_then(|ty| self.registry.get(ty))
            .expect(&format!(
                "Asset type {} not registered",
                std::any::type_name::<A>()
            ));

        let name = name.to_string();
        let id = AssetId::from(uuid::Uuid::new_v5(self.id.uuid(), name.as_bytes()));
        let path = self.path.with_name(name).into_owned();
        let meta = ArtifactMeta::new_child(id, asset_meta.ty, path, self.id)
            .with_unload_action(asset_meta.dependency_unload_action);
        let artifact = Artifact::new(&child, meta)?;

        self.children.push(artifact);

        Ok(id)
    }

    pub async fn read_asset(&self) -> Result<Vec<u8>, AssetIoError> {
        let mut buf = vec![];
        let mut reader = self.source.reader(self.path.path()).await?;
        AsyncReader::read_to_end(&mut reader, &mut buf)
            .await
            .map(|_| buf)
    }

    pub async fn read_metadata(&self) -> Result<Vec<u8>, AssetIoError> {
        let mut buf = vec![];
        let mut reader = self.source.reader(&self.metadata_path).await?;
        AsyncReader::read_to_end(&mut reader, &mut buf)
            .await
            .map(|_| buf)
    }
}

pub trait DynMetadata: downcast_rs::Downcast + Send + Sync + 'static {
    fn erased_id(&self) -> ErasedId;
}

downcast_rs::impl_downcast!(DynMetadata);

impl<S: Settings> DynMetadata for AssetMetadata<S> {
    fn erased_id(&self) -> ErasedId {
        self.id
    }
}

pub trait AssetImporter: Send + Sync + 'static {
    type Asset: Asset + Serialize + for<'a> Deserialize<'a>;

    type Settings: Settings + Serialize + for<'a> Deserialize<'a>;

    type Error: std::error::Error + Send + Sync + 'static;

    fn import(
        ctx: &mut ImportContext<'_>,
        reader: &mut dyn AsyncReader,
        metadata: &AssetMetadata<Self::Settings>,
    ) -> impl Future<Output = Result<Self::Asset, Self::Error>> + Send;

    fn extensions() -> &'static [&'static str] {
        &[]
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ErasedImporter {
    import: for<'a> fn(
        ImportContext<'a>,
        &'a mut dyn AsyncReader,
        &'a Box<dyn DynMetadata>,
    ) -> BoxFuture<'a, Result<Vec<Artifact>, BoxedError>>,
    deserialize_metadata: fn(&[u8]) -> Result<Box<dyn DynMetadata>, AssetIoError>,
    default_metadata: fn() -> Box<dyn DynMetadata>,
    type_id: fn() -> TypeId,
    asset_type_id: fn() -> TypeId,
    extensions: fn() -> &'static [&'static str],
}

impl ErasedImporter {
    pub fn new<I: AssetImporter>() -> Self {
        Self {
            import: |mut ctx, reader, metadata| {
                let f = async {
                    let metadata = metadata
                        .as_any()
                        .downcast_ref::<AssetMetadata<I::Settings>>()
                        .expect("AssetMetadata type mismatch");

                    let asset = <I as AssetImporter>::import(&mut ctx, reader, metadata)
                        .await
                        .map_err(|e| Box::new(e) as BoxedError)?;

                    let checksum = {
                        let asset = ctx.read_asset().await?;
                        let metadata = ctx
                            .source
                            .save_metadata(ctx.path.path(), metadata)
                            .await
                            .map_err(|e| Box::new(e) as BoxedError)?;

                        ImportMeta::get_checksum(&asset, &metadata)
                    };

                    let mut dependencies = vec![];
                    asset.get(&mut dependencies);

                    let unload_action = ctx
                        .registry
                        .get(ctx.ty)
                        .and_then(|meta| meta.dependency_unload_action);

                    let meta = ArtifactMeta::new(
                        metadata.id,
                        ctx.ty,
                        ctx.path.clone().into_owned(),
                        ImportMeta::new(ctx.processor, checksum),
                    )
                    .with_dependencies(dependencies)
                    .with_unload_action(unload_action);

                    let mut artifacts =
                        vec![Artifact::new(&asset, meta).map_err(|e| Box::new(e) as BoxedError)?];

                    artifacts.extend(ctx.children);

                    Ok(artifacts)
                };

                Box::pin(f)
            },
            deserialize_metadata: |data| {
                deserialize::<AssetMetadata<I::Settings>>(data)
                    .map(|metadata| Box::new(metadata) as Box<dyn DynMetadata>)
            },
            default_metadata: || Box::new(AssetMetadata::<I::Settings>::default()),
            type_id: || TypeId::of::<I::Asset>(),
            asset_type_id: || TypeId::of::<I>(),
            extensions: <I as AssetImporter>::extensions,
        }
    }

    pub fn import<'a>(
        &'a self,
        ctx: ImportContext<'a>,
        reader: &'a mut dyn AsyncReader,
        metadata: &'a Box<dyn DynMetadata>,
    ) -> BoxFuture<'a, Result<Vec<Artifact>, BoxedError>> {
        (self.import)(ctx, reader, metadata)
    }

    pub fn deserialize_metadata(&self, data: &[u8]) -> Result<Box<dyn DynMetadata>, AssetIoError> {
        match (self.deserialize_metadata)(data) {
            Ok(meta) => Ok(meta),
            Err(error) => Err(error),
        }
    }

    pub fn default_metadata(&self) -> Box<dyn DynMetadata> {
        (self.default_metadata)()
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

#[derive(Debug)]
pub struct AssetImporters {
    importers: Vec<ErasedImporter>,
    ty_map: HashMap<TypeId, Vec<usize>>,
    ext_map: HashMap<&'static str, usize>,
}

impl AssetImporters {
    pub fn new() -> Self {
        Self {
            importers: Vec::new(),
            ext_map: HashMap::new(),
            ty_map: HashMap::new(),
        }
    }

    pub fn get(&self, ty: TypeId) -> Option<Vec<&ErasedImporter>> {
        self.ty_map.get(&ty).map(|indicies| {
            indicies
                .iter()
                .map(|i| &self.importers[*i])
                .collect::<Vec<_>>()
        })
    }

    pub fn get_by_ext(&self, ext: &str) -> Option<&ErasedImporter> {
        self.ext_map.get(ext).map(|index| &self.importers[*index])
    }

    pub fn add<I: AssetImporter>(&mut self) {
        let ty = TypeId::of::<I>();
        let asset_ty = TypeId::of::<I::Asset>();
        if !self.contains(ty, asset_ty) {
            let index = self.importers.len();
            self.importers.push(ErasedImporter::new::<I>());
            self.ty_map.entry(asset_ty).or_default().push(index);
            for ext in I::extensions() {
                self.ext_map.insert(ext, index);
            }
        }
    }

    pub fn contains(&self, ty: TypeId, asset_ty: TypeId) -> bool {
        self.ty_map
            .get(&asset_ty)
            .is_some_and(|importers| importers.iter().any(|i| self.importers[*i].type_id() == ty))
    }
}

#[derive(thiserror::Error, Debug, Event)]
pub enum ImportError {
    #[error("{0}")]
    File(AssetIoError),

    #[error("{0}")]
    Folder(AssetIoError),

    #[error("Source: {name} {error}")]
    Source {
        name: SourceName<'static>,
        error: AssetIoError,
    },

    #[error("{0}")]
    LoadAsset(AssetIoError),

    #[error("{0}")]
    LoadArtifact(AssetIoError),

    #[error("{0}")]
    LoadMetadata(AssetIoError),

    #[error("{0}")]
    ImportAsset(BoxedError),

    #[error("{0}")]
    SaveAsset(AssetIoError),

    #[error("{0}")]
    ProcessAsset(BoxedError),

    #[error("{0}")]
    Unknown(AssetIoError),
}

impl From<(SourceName<'static>, AssetIoError)> for ImportError {
    fn from(value: (SourceName<'static>, AssetIoError)) -> Self {
        Self::Source {
            name: value.0,
            error: value.1,
        }
    }
}
