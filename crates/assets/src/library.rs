// use crate::{
//     asset::{Asset, AssetType, ErasedId, Settings},
//     io::serialize,
// };
// use serde::{Deserialize, Serialize};
// use std::hash::Hash;

// #[derive(Clone, Copy, Serialize, Deserialize)]
// pub struct ArtifactHeader {
//     pub id: ErasedId,
//     pub ty: AssetType,
//     pub checksum: u32,
//     pub full_checksum: u32,
//     pub dependencies: u32,
//     pub asset: u32,
//     pub settings: u32,
// }

// #[derive(Clone, Serialize, Deserialize)]
// pub struct Artifact {
//     header: ArtifactHeader,
//     data: Vec<u8>,
// }

// impl Artifact {
//     pub fn new<A: Asset + Serialize, S: Settings + Serialize>(
//         asset: &A,
//         settings: &S,
//         id: ErasedId,
//         // ty: AssetType,
//         dependencies: &Vec<ErasedId>,
//     ) -> Self {
//         let mut data = serialize(&dependencies).unwrap();
//         let asset_bytes = serialize(asset).unwrap();
//         let settings_bytes = serialize(settings).unwrap();

//         let checksum = {
//             let mut hasher = crc32fast::Hasher::new();
//             asset_bytes.hash(&mut hasher);
//             settings_bytes.hash(&mut hasher);
//             hasher.finalize()
//         };

//         let header = ArtifactHeader {
//             id,
//             ty,
//             checksum,
//             full_checksum: 0,
//             dependencies: data.len() as u32,
//             asset: asset_bytes.len() as u32,
//             settings: settings_bytes.len() as u32,
//         };

//         data.extend(asset_bytes);
//         data.extend(settings_bytes);

//         Self { header, data }
//     }

//     pub fn header(&self) -> ArtifactHeader {
//         self.header
//     }

//     pub fn header_mut(&mut self) -> &mut ArtifactHeader {
//         &mut self.header
//     }

//     pub fn data(&self) -> &[u8] {
//         &self.data
//     }
// }

// #[allow(unused_imports, dead_code)]
// mod import {
//     use std::{
//         hash::Hash,
//         path::{Path, PathBuf},
//     };

//     use futures::StreamExt;
//     use smol::io::AsyncReadExt;

//     use crate::{
//         asset::ErasedId,
//         io::{
//             AssetFuture, AssetReader,
//             cache::AssetLibrary,
//             deserialize,
//             source::{AssetPath, AssetSource, AssetSources},
//         },
//     };

//     use super::ArtifactHeader;

//     pub struct AssetCache {
//         source: AssetSource,
//     }

//     impl AssetCache {
//         pub fn artifact_path(&self, id: ErasedId) -> PathBuf {
//             todo!()
//         }

//         pub fn artifact_reader<'a>(
//             &'a self,
//             path: &'a Path,
//         ) -> AssetFuture<'a, Box<dyn AssetReader>> {
//             self.source.reader(path)
//         }
//     }

//     async fn refresh_assets(
//         mut assets: Vec<AssetPath>,
//         library: AssetLibrary,
//         sources: AssetSources,
//         cache: AssetCache,
//     ) {
//         while let Some(path) = assets.pop() {
//             let name = path.source();
//             let Some(source) = sources.get(name) else {
//                 continue;
//             };

//             let Ok(is_dir) = source.is_dir(path.path()).await else {
//                 continue;
//             };

//             if is_dir {
//                 let Ok(entries) = source.read_dir(path.path()).await else {
//                     continue;
//                 };

//                 let paths = entries
//                     .map(|p| AssetPath::new(name.clone(), p))
//                     .collect::<Vec<_>>()
//                     .await;
//                 assets.extend(paths);
//             } else {
//                 if matches!(path.ext(), Some("meta") | None) {
//                     continue;
//                 }

//                 // Check if asset path exists in library
//                 if library.get_id(&path).is_none() {
//                     continue;
//                 };

//                 // Check if asset metadata exists
//                 let meta_path = path.path().with_extension("meta");
//                 if !source.exists(&meta_path).await.unwrap_or(false) {
//                     continue;
//                 }

//                 let checksum = {
//                     let Ok(asset) = source.read_asset_bytes(path.path()).await else {
//                         continue;
//                     };

//                     let Ok(metadata) = source.read_metadata_bytes(path.path()).await else {
//                         continue;
//                     };

//                     let mut hasher = crc32fast::Hasher::new();
//                     asset.hash(&mut hasher);
//                     metadata.hash(&mut hasher);

//                     hasher.finalize()
//                 };

//                 let Ok(mut reader) = cache.artifact_reader(path.path()).await else {
//                     continue;
//                 };

//                 let Ok(header) = get_artifact_header(&mut reader).await else {
//                     continue;
//                 };

//                 // Check if asset or metadata has changed
//                 if checksum != header.checksum {}

//                 let Ok(full_checksum) = get_full_checksum(&cache, &mut reader, &header).await
//                 else {
//                     continue;
//                 };

//                 // Check if asset dependencies have changed
//                 if full_checksum != header.full_checksum {}
//             }
//         }
//     }

//     async fn get_artifact_header(
//         reader: &mut Box<dyn AssetReader>,
//     ) -> Result<ArtifactHeader, crate::io::AssetIoError> {
//         let mut buffer = [0u8; std::mem::size_of::<ArtifactHeader>()];
//         reader.read_exact(&mut buffer).await?;
//         deserialize::<ArtifactHeader>(&buffer)
//     }

//     async fn get_full_checksum(
//         cache: &AssetCache,
//         reader: &mut Box<dyn AssetReader>,
//         header: &ArtifactHeader,
//     ) -> Result<u32, crate::io::AssetIoError> {
//         let mut buffer = vec![0u8; header.dependencies as usize];
//         reader.read_exact(&mut buffer).await?;

//         let mut hasher = crc32fast::Hasher::new();
//         for id in deserialize::<Vec<ErasedId>>(&buffer)? {
//             let path = cache.artifact_path(id);
//             let Ok(mut reader) = cache.artifact_reader(&path).await else {
//                 continue;
//             };

//             let Ok(header) = get_artifact_header(&mut reader).await else {
//                 continue;
//             };

//             header.full_checksum.hash(&mut hasher);
//         }

//         Ok(hasher.finalize())
//     }
// }
