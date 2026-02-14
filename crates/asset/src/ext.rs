use serde::{Serialize, de::DeserializeOwned};
use std::{
    borrow::Cow,
    path::{Path, PathBuf},
};

pub trait SerializeExt {
    fn to_bytes(&self) -> Result<Vec<u8>, bincode::error::EncodeError>;
}

impl<T: Serialize> SerializeExt for T {
    fn to_bytes(&self) -> Result<Vec<u8>, bincode::error::EncodeError> {
        bincode::serde::encode_to_vec(self, bincode::config::standard())
    }
}

pub trait DeserializeExt: Sized {
    fn from_bytes(bytes: &[u8]) -> Result<Self, bincode::error::DecodeError>;
}

impl<T: DeserializeOwned> DeserializeExt for T {
    fn from_bytes(bytes: &[u8]) -> Result<Self, bincode::error::DecodeError> {
        bincode::serde::decode_from_slice::<T, _>(bytes, bincode::config::standard())
            .map(|(v, _)| v)
    }
}

pub trait PathExt {
    fn ext(&self) -> Option<&str>;
    fn append_ext(&self, ext: &str) -> PathBuf;
    fn with_prefix(&'_ self, prefix: impl AsRef<Path>) -> Cow<'_, Path>;
    fn without_prefix(&self, prefix: impl AsRef<Path>) -> &Path;
}

impl<T: AsRef<Path>> PathExt for T {
    fn ext(&self) -> Option<&str> {
        self.as_ref().extension().and_then(|ext| ext.to_str())
    }
    fn append_ext(&self, ext: &str) -> PathBuf {
        let path = self.as_ref().to_path_buf();
        format!("{}.{}", path.display(), ext).into()
    }

    fn with_prefix(&'_ self, prefix: impl AsRef<Path>) -> Cow<'_, Path> {
        match self.as_ref().starts_with(prefix.as_ref()) {
            false => Cow::Owned(prefix.as_ref().join(self)),
            true => Cow::Borrowed(self.as_ref()),
        }
    }

    fn without_prefix(&self, prefix: impl AsRef<Path>) -> &Path {
        let path = self.as_ref();
        let prefix = prefix.as_ref();
        path.strip_prefix(prefix).unwrap_or(path)
    }
}
