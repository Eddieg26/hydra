use serde::{Deserialize, Serialize};
use std::{
    borrow::Cow,
    path::{Path, PathBuf},
};

use crate::asset::ErasedId;

#[derive(Clone, Default, Debug, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub enum AssetSource<'a> {
    #[default]
    Default,
    Name(Cow<'a, str>),
}

impl<'a> AssetSource<'a> {
    fn to_owned(&self) -> AssetSource<'static> {
        match self {
            AssetSource::Default => AssetSource::Default,
            AssetSource::Name(v) => AssetSource::Name(Cow::Owned(v.to_string())),
        }
    }
}

impl<'a> From<&'a str> for AssetSource<'a> {
    fn from(value: &'a str) -> Self {
        Self::Name(value.into())
    }
}

impl From<String> for AssetSource<'static> {
    fn from(value: String) -> Self {
        Self::Name(value.into())
    }
}

impl std::fmt::Display for AssetSource<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AssetSource::Default => write!(f, "default"),
            AssetSource::Name(name) => write!(f, "{}", name),
        }
    }
}

impl std::error::Error for AssetSource<'_> {}

/// Represents a path to an asset, which can include a source and an optional name.
/// An asset path can be in the following formats:
/// * texture.png
/// * path/to/texture.png
/// * path/to/texture.png@name
/// * source://path/to/asset@name
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AssetPath<'a> {
    source: AssetSource<'a>,
    path: Cow<'a, Path>,
    name: Option<Cow<'a, str>>,
}

impl<'a> AssetPath<'a> {
    pub fn new(source: AssetSource<'a>, path: &'a str) -> Self {
        Self {
            source,
            path: Cow::Borrowed(path.as_ref()),
            name: None,
        }
    }

    pub fn new_owned(source: AssetSource<'a>, path: impl AsRef<Path> + ToOwned) -> Self {
        Self {
            source,
            path: Cow::Owned(path.as_ref().to_owned()),
            name: None,
        }
    }

    pub fn with_name(mut self, name: impl Into<Cow<'a, str>>) -> Self {
        self.name = Some(name.into());
        self
    }

    pub fn from_path(path: &'a Path) -> Option<Self> {
        let path = path.to_str()?;
        Some(Self::parse(path))
    }

    pub fn source(&self) -> &AssetSource<'a> {
        &self.source
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    pub fn to_owned(&self) -> AssetPath<'static> {
        let source = self.source.to_owned();
        let path = Cow::Owned(self.path.clone().into_owned());
        let name = self.name.clone().map(|v| Cow::Owned(v.to_string()));

        AssetPath { source, path, name }
    }

    fn parse(path: &'a str) -> Self {
        let (source, start) = match path.find("://") {
            Some(index) => (AssetSource::from(&path[..index]), index + 3),
            None => (AssetSource::Default, 0),
        };

        let (name, end) = match path[start..].find('@') {
            Some(index) => {
                let name = &path[start + index + 1..];
                (Some(Cow::from(name)), start + index)
            }
            None => (None, path.len()),
        };

        let path = Cow::from(Path::new(&path[start..end]));

        Self { source, path, name }
    }
}

impl<'a> From<&'a str> for AssetPath<'a> {
    fn from(path: &'a str) -> Self {
        Self::parse(path)
    }
}

impl From<String> for AssetPath<'static> {
    fn from(path: String) -> Self {
        AssetPath::parse(&path).to_owned()
    }
}

impl From<PathBuf> for AssetPath<'static> {
    fn from(value: PathBuf) -> Self {
        let path = format!("{}", value.display());
        AssetPath::parse(&path).to_owned()
    }
}

impl std::ops::Deref for AssetPath<'_> {
    type Target = Path;

    fn deref(&self) -> &Self::Target {
        &self.path
    }
}

impl AsRef<Path> for AssetPath<'_> {
    fn as_ref(&self) -> &Path {
        &self.path
    }
}

impl From<&AssetPath<'_>> for PathBuf {
    fn from(value: &AssetPath) -> Self {
        let path = match value.source {
            AssetSource::Default => format!("{}", value.path.display()),
            AssetSource::Name(ref name) => format!("{}://{}", name, value.path.display()),
        };

        match &value.name {
            Some(name) => PathBuf::from(format!("{}@{}", path, &name)),
            None => PathBuf::from(path),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum LoadPath<'a> {
    Path(AssetPath<'a>),
    Id(ErasedId),
}

impl<'a> From<AssetPath<'a>> for LoadPath<'a> {
    fn from(value: AssetPath<'a>) -> Self {
        Self::Path(value)
    }
}

impl<'a> From<&'a str> for LoadPath<'a> {
    fn from(value: &'a str) -> Self {
        Self::Path(AssetPath::from(value))
    }
}

impl From<String> for LoadPath<'static> {
    fn from(value: String) -> Self {
        Self::Path(AssetPath::from(value))
    }
}

impl<I: Into<ErasedId>> From<I> for LoadPath<'static> {
    fn from(value: I) -> Self {
        Self::Id(value.into())
    }
}

impl std::fmt::Display for LoadPath<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LoadPath::Path(path) => write!(f, "{:?}", path),
            LoadPath::Id(id) => write!(f, "{}", id.to_string()),
        }
    }
}

#[allow(unused_imports)]
mod tests {
    use crate::io::{AssetPath, AssetSource};
    use std::{borrow::Cow, path::PathBuf};

    #[test]
    fn test_asset_path() {
        let path = AssetPath::from(PathBuf::from("test.txt"));
        assert_eq!(AssetSource::Default, path.source);
        assert_eq!(PathBuf::from(&path), PathBuf::from("test.txt"));
        assert_eq!(None, path.name());

        let path = AssetPath::from(PathBuf::from("models/test.obj"));
        assert_eq!(AssetSource::Default, path.source);
        assert_eq!(PathBuf::from(&path), PathBuf::from("models/test.obj"));
        assert_eq!(None, path.name());

        let path = AssetPath::from(PathBuf::from("models/test.obj@cube"));
        assert_eq!(AssetSource::Default, path.source);
        assert_eq!(PathBuf::from(&path), PathBuf::from("models/test.obj@cube"));
        assert_eq!(Some("cube"), path.name());

        let path = AssetPath::from(PathBuf::from("remote://models/test.obj@cube"));
        assert_eq!(AssetSource::Name(Cow::Borrowed("remote")), path.source);
        assert_eq!(PathBuf::from(&path), PathBuf::from("remote://models/test.obj@cube"));
        assert_eq!(Some("cube"), path.name());
    }
}
