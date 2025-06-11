use super::{
    AssetIoError, AssetPath, AsyncReader, ErasedFileSystem, FileSystem, deserialize, serialize,
};
use crate::asset::{Asset, AssetAction, AssetId, AssetType, ErasedId};
use serde::{Deserialize, Serialize, ser::SerializeStruct};
use smol::io::{AsyncReadExt, AsyncWriteExt};
use std::{
    collections::HashMap,
    hash::Hash,
    path::{Path, PathBuf},
    sync::Arc,
};

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct AssetLibrary {
    id_map: HashMap<AssetPath<'static>, ErasedId>,
    path_map: HashMap<ErasedId, AssetPath<'static>>,
}

impl AssetLibrary {
    pub fn new() -> Self {
        Self {
            id_map: HashMap::new(),
            path_map: HashMap::new(),
        }
    }

    pub fn get_id<'a>(&'a self, path: &'a AssetPath<'a>) -> Option<&'a ErasedId> {
        self.id_map.get(path)
    }

    pub fn get_path(&self, id: ErasedId) -> Option<&AssetPath<'static>> {
        self.path_map.get(&id)
    }

    pub fn add_asset(&mut self, path: AssetPath<'static>, id: ErasedId) {
        self.id_map.insert(path.clone(), id);
        self.path_map.insert(id, path);
    }

    pub fn remove_asset(&mut self, path: &AssetPath<'static>) -> Option<ErasedId> {
        self.id_map.remove(path).and_then(|id| {
            self.path_map.remove(&id);
            Some(id)
        })
    }

    pub fn len(&self) -> usize {
        self.id_map.len()
    }

    pub fn replace(&mut self, library: Self) {
        self.id_map = library.id_map;
        self.path_map = library.path_map;
    }

    pub fn clear(&mut self) {
        self.id_map.clear();
        self.path_map.clear();
    }
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct ImportMeta {
    pub processor: Option<u32>,
    pub checksum: u32,
    pub full_checksum: u32,
    pub dependencies: Vec<(ErasedId, u32)>,
}

impl ImportMeta {
    pub fn new(processor: Option<u32>, checksum: u32) -> Self {
        Self {
            processor,
            checksum,
            full_checksum: Self::get_full_checksum(checksum, std::iter::empty()),
            dependencies: vec![],
        }
    }

    pub fn get_checksum(asset: &[u8], metadata: &[u8]) -> u32 {
        let mut hasher = crc32fast::Hasher::new();
        asset.hash(&mut hasher);
        metadata.hash(&mut hasher);

        hasher.finalize()
    }

    pub fn get_full_checksum(checksum: u32, dependencies: impl Iterator<Item = u32>) -> u32 {
        let mut hasher = crc32fast::Hasher::new();
        checksum.hash(&mut hasher);

        for checksum in dependencies {
            checksum.hash(&mut hasher);
        }

        hasher.finalize()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactMeta {
    pub id: ErasedId,
    pub ty: AssetType,
    pub path: AssetPath<'static>,
    pub import: ImportMeta,
    pub dependencies: Vec<ErasedId>,
    pub children: Vec<ErasedId>,
    pub parent: Option<ErasedId>,
    pub unload_action: Option<AssetAction>,
}

impl ArtifactMeta {
    pub fn new(
        id: impl Into<ErasedId>,
        ty: AssetType,
        path: AssetPath<'static>,
        import: ImportMeta,
    ) -> Self {
        Self {
            id: id.into(),
            ty,
            path,
            import,
            dependencies: vec![],
            children: vec![],
            parent: None,
            unload_action: None,
        }
    }

    pub fn new_child(
        id: impl Into<ErasedId>,
        ty: AssetType,
        path: AssetPath<'static>,
        parent: ErasedId,
    ) -> Self {
        Self {
            id: id.into(),
            ty,
            path,
            import: ImportMeta::new(None, 0),
            dependencies: vec![],
            children: vec![],
            parent: Some(parent),
            unload_action: None,
        }
    }

    pub fn with_dependencies(mut self, dependencies: Vec<ErasedId>) -> Self {
        self.dependencies = dependencies;
        self
    }

    pub fn with_children(mut self, children: Vec<ErasedId>) -> Self {
        self.children = children;
        self
    }

    pub fn with_unload_action(mut self, action: impl Into<Option<AssetAction>>) -> Self {
        self.unload_action = action.into();
        self
    }
}

#[derive(Debug, Clone)]
pub struct Artifact {
    pub(crate) meta: ArtifactMeta,
    pub(crate) data: Vec<u8>,
}

impl Artifact {
    pub fn new<A: Asset + Serialize>(asset: &A, meta: ArtifactMeta) -> Result<Self, AssetIoError> {
        let data = serialize(&asset)?;

        Ok(Self { meta, data })
    }

    pub fn from_raw_parts(meta: ArtifactMeta, data: Vec<u8>) -> Self {
        Self { meta, data }
    }

    pub fn id(&self) -> ErasedId {
        self.meta.id
    }

    pub fn ty(&self) -> AssetType {
        self.meta.ty
    }

    pub fn path(&self) -> &AssetPath<'static> {
        &self.meta.path
    }

    pub fn meta(&self) -> &ArtifactMeta {
        &self.meta
    }

    pub fn data(&self) -> &[u8] {
        &self.data
    }

    pub fn asset<A: Asset + for<'a> Deserialize<'a>>(&self) -> Result<A, AssetIoError> {
        deserialize(&self.data)
    }
}

impl Serialize for Artifact {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut artifact = serializer.serialize_struct("Artifact", 3)?;
        let meta = serialize(&self.meta).map_err(serde::ser::Error::custom)?;
        let size: [u8; 4] = (meta.len() as u32).to_le_bytes();

        artifact.serialize_field("header", &size)?;
        artifact.serialize_field("meta", &self.meta)?;
        artifact.serialize_field("data", &self.data)?;

        artifact.end()
    }
}

impl<'de> Deserialize<'de> for Artifact {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct Visitor;

        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = Artifact;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("struct Artifact")
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::SeqAccess<'de>,
            {
                use serde::de::Error;

                let _ = seq
                    .next_element::<[u8; 4]>()?
                    .ok_or(Error::custom("Expected meta size"))?;

                let meta = seq
                    .next_element::<ArtifactMeta>()?
                    .ok_or(Error::custom("Expected meta"))?;

                let data = seq
                    .next_element::<Vec<u8>>()?
                    .ok_or(Error::custom("Expected data bytes"))?;

                Ok(Artifact { meta, data })
            }
        }

        deserializer.deserialize_struct("Artifact", &["meta_size", "meta", "data"], Visitor)
    }
}

pub struct ArtifactReader(Box<dyn AsyncReader>);

impl ArtifactReader {
    pub fn new(reader: Box<dyn AsyncReader>) -> Self {
        Self(reader)
    }

    pub async fn meta(&mut self) -> Result<ArtifactMeta, AssetIoError> {
        let mut header_buf = [0u8; 4];
        self.0.read_exact(&mut header_buf).await?;

        let meta_size = u32::from_le_bytes(header_buf) as usize;
        let mut meta_buf = vec![0u8; meta_size + 1];
        self.0.read_exact(&mut meta_buf).await?;

        deserialize(&meta_buf)
    }

    pub async fn data(mut self) -> Result<Vec<u8>, AssetIoError> {
        let mut buf = vec![];
        AsyncReader::read_to_end(&mut self.0, &mut buf)
            .await
            .map(|_| buf)
    }

    pub async fn into_meta(mut self) -> Result<ArtifactMeta, AssetIoError> {
        self.meta().await
    }

    pub async fn into_artifact(mut self) -> Result<Artifact, AssetIoError> {
        let mut buf = vec![];
        AsyncReader::read_to_end(&mut self.0, &mut buf).await?;

        deserialize(&buf)
    }
}

#[derive(Clone)]
pub struct AssetCache {
    fs: Arc<dyn ErasedFileSystem>,
    artifacts: PathBuf,
    temp: PathBuf,
    library: PathBuf,
}

impl AssetCache {
    pub fn new<F: FileSystem>(fs: F) -> Self {
        let fs = Arc::new(fs);
        let artifacts = fs.root().join("artifacts");
        let temp = fs.root().join(".temp");
        let library = fs.root().join("assets.lib");

        Self {
            fs,
            artifacts,
            temp,
            library,
        }
    }

    pub fn fs(&self) -> &dyn ErasedFileSystem {
        self.fs.as_ref()
    }

    pub fn get_artifacts_path(&self) -> &Path {
        &self.artifacts
    }

    pub fn get_temp_path(&self) -> &Path {
        &self.temp
    }

    pub fn get_library_path(&self) -> &Path {
        &self.library
    }

    pub fn get_artifact_path(&self, id: ErasedId) -> PathBuf {
        self.artifacts.join(id.to_string())
    }

    pub fn get_temp_artifact_path(&self, id: ErasedId) -> PathBuf {
        self.temp.join(id.to_string())
    }

    pub async fn get_artifact(&self, id: ErasedId) -> Result<Artifact, AssetIoError> {
        let path = self.get_artifact_path(id);
        let mut data = vec![];
        let mut reader = self.fs.reader(&path).await?;
        AsyncReader::read_to_end(&mut reader, &mut data).await?;
        deserialize::<Artifact>(&data)
    }

    pub async fn get_artifact_reader(&self, id: ErasedId) -> Result<ArtifactReader, AssetIoError> {
        let path = self.get_artifact_path(id);
        self.fs.reader(&path).await.map(ArtifactReader::new)
    }

    pub async fn get_temp_artifact(&self, id: ErasedId) -> Result<Artifact, AssetIoError> {
        let path = self.get_temp_artifact_path(id);
        let mut data = vec![];
        let mut reader = self.fs.reader(&path).await?;
        AsyncReader::read_to_end(&mut reader, &mut data).await?;

        deserialize::<Artifact>(&data)
    }

    pub async fn save_artifact(&self, artifact: &Artifact) -> Result<usize, AssetIoError> {
        let data = serialize(artifact)?;
        let path = self.get_artifact_path(artifact.id());
        let mut writer = self.fs.writer(&path).await?;
        writer.write(&data).await.map_err(AssetIoError::from)
    }

    pub async fn save_temp_artifact(&self, artifact: &Artifact) -> Result<usize, AssetIoError> {
        let data = serialize(artifact)?;
        let path = self.get_temp_artifact_path(artifact.id());
        let mut writer = self.fs.writer(&path).await?;
        writer.write(&data).await.map_err(AssetIoError::from)
    }

    pub async fn remove_artifact(&self, id: ErasedId) -> Result<(), AssetIoError> {
        let path = self.get_artifact_path(id);
        self.fs.remove(&path).await
    }

    pub async fn remove_temp_artifact(&self, id: ErasedId) -> Result<(), AssetIoError> {
        let path = self.get_temp_artifact_path(id);
        self.fs.remove(&path).await
    }

    pub async fn load_asset<A: Asset + for<'a> Deserialize<'a>>(
        &self,
        id: AssetId<A>,
    ) -> Result<LoadedAsset<A>, AssetIoError> {
        let artifact = self
            .get_artifact_reader(id.into())
            .await?
            .into_artifact()
            .await?;
        deserialize::<A>(artifact.data()).map(|asset| LoadedAsset {
            asset,
            meta: artifact.meta,
        })
    }

    pub async fn load_library(&self) -> Result<AssetLibrary, AssetIoError> {
        let mut reader = self.fs.reader(&self.library).await?;
        let mut buf = vec![];
        AsyncReader::read_to_end(&mut reader, &mut buf).await?;

        deserialize(&buf)
    }

    pub async fn save_library(&self, library: &AssetLibrary) -> Result<usize, AssetIoError> {
        let data = serialize(library)?;
        let mut writer = self.fs.writer(&self.library).await?;
        writer.write(&data).await.map_err(AssetIoError::from)
    }

    pub async fn create_root(&self) -> Result<(), AssetIoError> {
        if !self.fs.exists(&self.temp).await? {
            self.fs.create_dir_all(&self.temp).await
        } else {
            Ok(())
        }
    }

    pub async fn create_artifacts_path(&self) -> Result<(), AssetIoError> {
        if !self.fs.exists(&self.artifacts).await? {
            self.fs.create_dir_all(&self.artifacts).await
        } else {
            Ok(())
        }
    }

    pub async fn create_temp(&self) -> Result<(), AssetIoError> {
        if !self.fs.exists(&self.temp).await? {
            self.fs.create_dir_all(&self.temp).await
        } else {
            Ok(())
        }
    }

    pub async fn delete_temp(&self) -> Result<(), AssetIoError> {
        if self.fs.exists(&self.temp).await? {
            self.fs.remove_dir(&self.temp).await
        } else {
            Ok(())
        }
    }
}

impl std::fmt::Debug for AssetCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AssetCache")
            .field("artifacts", &self.artifacts)
            .field("temp", &self.temp)
            .field("library", &self.library)
            .finish()
    }
}

pub struct LoadedAsset<A: Asset> {
    pub asset: A,
    pub meta: ArtifactMeta,
}

#[allow(unused_imports, dead_code)]
mod tests {
    use crate::{
        Asset, AssetType, ErasedId,
        io::{
            Artifact, ArtifactMeta, AssetPath, ImportMeta, SourceName, VirtualFs, deserialize,
            serialize,
        },
    };
    use serde::{Deserialize, Serialize};

    use super::AssetCache;

    #[derive(Serialize, Deserialize, Asset, Debug, PartialEq, Eq)]
    struct TestAsset {
        id: u32,
        name: String,
    }

    #[test]
    fn test_serialize_artifact() {
        let asset = TestAsset {
            id: 1,
            name: "Test Asset".to_string(),
        };

        let meta = ArtifactMeta::new(
            ErasedId::NONE,
            AssetType::NONE,
            AssetPath::new(SourceName::Default, "test_asset"),
            ImportMeta::new(None, 123),
        );

        let artifact = Artifact::new(&asset, meta).expect("Failed to create artifact");
        let serialized = serialize(&artifact).expect("Failed to serialize artifact");
        let artifact =
            deserialize::<Artifact>(&serialized).expect("Failed to deserialize artifact");

        let test_asset = artifact
            .asset::<TestAsset>()
            .expect("Failed to deserialize asset");

        assert_eq!(test_asset, asset);
    }

    #[test]
    fn test_artifact_reader() {
        let fs = VirtualFs::new();
        let cache = AssetCache::new(fs);
        smol::block_on(async {
            let asset = TestAsset {
                id: 1,
                name: "Test Asset".to_string(),
            };

            let meta = ArtifactMeta::new(
                ErasedId::NONE,
                AssetType::NONE,
                AssetPath::new(SourceName::Default, "test_asset"),
                ImportMeta::new(None, 123),
            );

            let artifact = Artifact::new(&asset, meta).expect("Failed to create artifact");
            cache.save_artifact(&artifact).await.unwrap();

            let mut reader = cache.get_artifact_reader(artifact.id()).await.unwrap();
            let meta = reader.meta().await.unwrap();

            assert_eq!(meta.id, artifact.id());
        });
    }
}
