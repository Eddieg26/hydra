use crate::{
    asset::{Asset, AssetMetadata, ErasedId, Settings},
    io::{
        AssetReader, BoxedFuture, deserialize,
        source::{AssetPath, AssetSource},
    },
};
use serde::{Deserialize, Serialize};
use std::any::TypeId;

pub struct ImportContext<'a> {
    path: &'a AssetPath,
    source: &'a AssetSource,
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
        ctx: &mut ImportContext,
        reader: &mut dyn AssetReader,
        metadata: &AssetMetadata<Self::Settings>,
    ) -> impl Future<Output = Result<Self::Asset, Self::Error>>;

    fn extensions() -> &'static [&'static str] {
        &[]
    }
}

pub struct ImportedAsset {
    data: Vec<u8>,
}

impl ImportedAsset {
    pub fn new<A: Asset>(asset: A) -> Self {
        todo!()
    }
}

pub type BoxedError = Box<dyn std::error::Error + Send + Sync + 'static>;

pub struct ErasedImporter {
    import: for<'a> fn(
        &'a mut ImportContext,
        &'a mut dyn AssetReader,
        &'a Box<dyn DynMetadata>,
    ) -> BoxedFuture<'a, ImportedAsset, BoxedError>,
    deserialize_metadata: fn(&[u8]) -> Result<Box<dyn DynMetadata>, BoxedError>,
    default_metadata: fn() -> Box<dyn DynMetadata>,
    type_id: fn() -> TypeId,
    asset_type_id: fn() -> TypeId,
    asset_name: fn() -> &'static str,
    extensions: fn() -> &'static [&'static str],
}

impl ErasedImporter {
    pub fn new<I: AssetImporter>() -> Self {
        Self {
            import: |ctx, reader, metadata| {
                let f = async move {
                    let metadata = metadata
                        .as_any()
                        .downcast_ref::<AssetMetadata<I::Settings>>()
                        .expect("AssetMetadata type mismatch");

                    match <I as AssetImporter>::import(ctx, reader, metadata).await {
                        Ok(asset) => Ok(ImportedAsset::new(asset)),
                        Err(error) => {
                            Err(Box::new(error) as Box<dyn std::error::Error + Send + Sync>)
                        }
                    }
                };

                Box::new(f)
            },
            deserialize_metadata: |data| match deserialize::<AssetMetadata<I::Settings>>(data) {
                Ok(metadata) => Ok(Box::new(metadata)),
                Err(error) => Err(Box::new(error)),
            },
            default_metadata: || Box::new(AssetMetadata::<I::Settings>::default()),
            type_id: || TypeId::of::<I::Asset>(),
            asset_type_id: || TypeId::of::<I>(),
            asset_name: || ecs::ext::short_type_name::<I::Asset>(),
            extensions: <I as AssetImporter>::extensions,
        }
    }

    pub fn import<'a>(
        &'a self,
        ctx: &'a mut ImportContext<'a>,
        reader: &'a mut dyn AssetReader,
        metadata: &'a Box<dyn DynMetadata>,
    ) -> BoxedFuture<'a, ImportedAsset, BoxedError> {
        (self.import)(ctx, reader, metadata)
    }

    pub fn deserialize_metadata(&self, data: &[u8]) -> Result<Box<dyn DynMetadata>, BoxedError> {
        (self.deserialize_metadata)(data)
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

    pub fn asset_name(&self) -> &'static str {
        (self.asset_name)()
    }

    pub fn extensions(&self) -> &'static [&'static str] {
        (self.extensions)()
    }
}

pub trait AssetProcessor: Send + Sync + 'static {
    type Importer: AssetImporter;

    type Error: std::error::Error + Send + Sync + 'static;
}
