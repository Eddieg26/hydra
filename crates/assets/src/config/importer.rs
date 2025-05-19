use super::BoxedError;
use crate::{
    asset::{Asset, AssetMetadata, AssetType, ErasedId, Settings},
    io::{
        Artifact, ArtifactMeta, AssetFuture, AssetIoError, AssetPath, AssetSource, AsyncReader,
        BoxedFuture, ImportMeta, PathExt, deserialize, serialize,
    },
};
use serde::{Deserialize, Serialize};
use std::{
    any::TypeId,
    collections::HashMap,
    hash::Hash,
    path::{Path, PathBuf},
};

pub struct ImportContext<'a> {
    ty: AssetType,
    source: &'a AssetSource,
    path: &'a AssetPath<'a>,
    metadata_path: PathBuf,
    processor: Option<u32>,
}

impl<'a> ImportContext<'a> {
    pub fn new(ty: AssetType, path: &'a AssetPath<'a>, source: &'a AssetSource) -> Self {
        let metadata_path = path.path().append_ext("meta");
        Self {
            ty,
            path,
            source,
            metadata_path,
            processor: None,
        }
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
        ctx: &mut ImportContext,
        reader: &mut dyn AsyncReader,
        metadata: &AssetMetadata<Self::Settings>,
    ) -> impl Future<Output = Result<Self::Asset, Self::Error>>;

    fn extensions() -> &'static [&'static str] {
        &[]
    }
}

#[derive(Clone, Copy)]
pub struct ErasedImporter {
    import: for<'a> fn(
        &'a mut ImportContext,
        &'a mut dyn AsyncReader,
        &'a Box<dyn DynMetadata>,
    ) -> AssetFuture<'a, Artifact, BoxedError>,
    deserialize_metadata: fn(&[u8]) -> Result<Box<dyn DynMetadata>, AssetIoError>,
    default_metadata: fn() -> Box<dyn DynMetadata>,
    type_id: fn() -> TypeId,
    asset_type_id: fn() -> TypeId,
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

                    let asset = <I as AssetImporter>::import(ctx, reader, metadata)
                        .await
                        .map_err(|e| Box::new(e) as BoxedError)?;

                    let checksum = {
                        let mut hasher = crc32fast::Hasher::new();
                        let mut data = ctx.read_asset().await?;
                        let metabytes =
                            serialize(metadata).map_err(|e| Box::new(e) as BoxedError)?;
                        data.extend(metabytes);
                        data.hash(&mut hasher);
                        hasher.finalize()
                    };

                    let mut dependencies = vec![];
                    asset.dependencies(&mut dependencies);

                    let meta = ArtifactMeta {
                        id: metadata.id,
                        ty: ctx.ty,
                        path: ctx.path.clone().into_static(),
                        import: ImportMeta::new(ctx.processor, checksum),
                        dependencies,
                    };

                    Artifact::new(&asset, meta).map_err(|e| Box::new(e) as BoxedError)
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
        ctx: &'a mut ImportContext<'a>,
        reader: &'a mut dyn AsyncReader,
        metadata: &'a Box<dyn DynMetadata>,
    ) -> AssetFuture<'a, Artifact, BoxedError> {
        (self.import)(ctx, reader, metadata)
    }

    pub fn deserialize_metadata(&self, data: &[u8]) -> Result<Box<dyn DynMetadata>, AssetIoError> {
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

    pub fn extensions(&self) -> &'static [&'static str] {
        (self.extensions)()
    }
}

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
