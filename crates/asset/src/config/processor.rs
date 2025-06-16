use super::{BoxedError, importer::AssetImporter};
use crate::{
    asset::{Asset, AssetId, AssetMetadata},
    io::{ArtifactMeta, AssetIoError, cache::AssetCache, deserialize, serialize},
};
use ecs::IndexMap;
use futures::future::BoxFuture;
use serde::{Deserialize, Serialize};
use std::{any::TypeId, collections::HashMap};

pub struct ProcessContext<'a> {
    cache: &'a AssetCache,
    pub(crate) dependencies: Vec<ArtifactMeta>,
}

impl<'a> ProcessContext<'a> {
    pub fn new(cache: &'a AssetCache) -> Self {
        Self {
            cache,
            dependencies: Vec::new(),
        }
    }

    pub async fn load<A: Asset + for<'de> Deserialize<'de>>(
        &mut self,
        id: impl Into<AssetId<A>>,
    ) -> Result<A, AssetIoError> {
        let loaded = self.cache.load_asset::<A>(id.into()).await?;
        self.dependencies.push(loaded.meta);

        Ok(loaded.asset)
    }
}

pub trait AssetProcessor: Send + Sync + 'static {
    type Input: AssetImporter;

    type Output: Asset + Serialize + for<'a> Deserialize<'a>;

    type Error: std::error::Error + Send + Sync + 'static;

    fn process(
        ctx: &mut ProcessContext<'_>,
        asset: <Self::Input as AssetImporter>::Asset,
        metadata: &AssetMetadata<<Self::Input as AssetImporter>::Settings>,
    ) -> impl Future<Output = Result<Self::Output, Self::Error>> + Send;
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ErasedProcesser {
    process: for<'a> fn(
        &'a mut ProcessContext,
        &'a [u8],
        Vec<u8>,
    ) -> BoxFuture<'a, Result<Vec<u8>, BoxedError>>,
    output_asset: fn() -> TypeId,
}

impl ErasedProcesser {
    pub fn new<P: AssetProcessor>() -> Self {
        Self {
            process: |ctx, asset, metadata| {
                let f = async move {
                    let metadata = deserialize::<
                        AssetMetadata<<P::Input as AssetImporter>::Settings>,
                    >(&metadata)?;

                    let asset = deserialize::<<P::Input as AssetImporter>::Asset>(asset)
                        .map_err(|e| Box::new(e) as BoxedError)?;

                    let asset = P::process(ctx, asset, &metadata)
                        .await
                        .map_err(|e| Box::new(e) as BoxedError)?;

                    serialize(&asset).map_err(|e| Box::new(e) as BoxedError)
                };

                Box::pin(f)
            },
            output_asset: || TypeId::of::<P::Output>(),
        }
    }

    pub fn process<'a>(
        &self,
        ctx: &'a mut ProcessContext,
        asset: &'a [u8],
        metadata: Vec<u8>,
    ) -> BoxFuture<'a, Result<Vec<u8>, BoxedError>> {
        (self.process)(ctx, asset, metadata)
    }

    pub fn output_asset(&self) -> TypeId {
        (self.output_asset)()
    }
}

#[derive(Debug)]
pub struct AssetProcessors {
    processors: IndexMap<TypeId, ErasedProcesser>,
    default_processors: HashMap<&'static str, TypeId>,
}

impl AssetProcessors {
    pub fn new() -> Self {
        Self {
            processors: IndexMap::new(),
            default_processors: HashMap::new(),
        }
    }

    pub fn get(&self, ty: TypeId) -> Option<&ErasedProcesser> {
        self.processors.get(&ty)
    }

    pub fn set_default<P: AssetProcessor>(&mut self) {
        let ty = TypeId::of::<P>();
        if !self.processors.contains_key(&ty) {
            self.processors.insert(ty, ErasedProcesser::new::<P>());
        }
        for ext in P::Input::extensions() {
            self.default_processors.insert(ext, ty);
        }
    }

    pub fn get_default(&self, ext: &str) -> Option<u32> {
        self.default_processors
            .get(ext)
            .and_then(|ty| self.processors.get_index_of(ty))
            .map(|i| i as u32)
    }

    pub fn add<P: AssetProcessor>(&mut self) {
        let ty = TypeId::of::<P>();
        if !self.processors.contains_key(&ty) {
            self.processors.insert(ty, ErasedProcesser::new::<P>());
        }
    }
}

impl std::ops::Index<u32> for AssetProcessors {
    type Output = ErasedProcesser;

    fn index(&self, index: u32) -> &Self::Output {
        &self.processors[index as usize]
    }
}
