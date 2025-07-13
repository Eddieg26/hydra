use crate::asset::ErasedId;
use serde::{Deserialize, Serialize};

pub trait Settings:
    Default + Send + Sync + Serialize + for<'de> Deserialize<'de> + 'static
{
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DefaultSettings {
    pub created: u64,
}

impl Default for DefaultSettings {
    fn default() -> Self {
        Self {
            created: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        }
    }
}

impl Settings for DefaultSettings {}

#[derive(Serialize)]
pub struct AssetSettings<S: Settings> {
    pub id: ErasedId,
    settings: S,
}
impl<S: Settings> AssetSettings<S> {
    pub fn new(id: impl Into<ErasedId>, settings: S) -> Self {
        Self {
            id: id.into(),
            settings,
        }
    }
}

impl<S: Settings> Default for AssetSettings<S> {
    fn default() -> Self {
        Self {
            id: ErasedId::new(),
            settings: S::default(),
        }
    }
}

impl<S: Settings> std::ops::Deref for AssetSettings<S> {
    type Target = S;

    fn deref(&self) -> &Self::Target {
        &self.settings
    }
}

impl<S: Settings> std::ops::DerefMut for AssetSettings<S> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.settings
    }
}

impl<'de, S: Settings> Deserialize<'de> for AssetSettings<S> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let (id, settings): (ErasedId, S) = Deserialize::deserialize(deserializer)?;
        Ok(Self {
            id: ErasedId::from(id),
            settings,
        })
    }
}

pub trait ErasedAssetSettings: downcast_rs::Downcast + Send + Sync + 'static {
    fn id(&self) -> ErasedId;
}
downcast_rs::impl_downcast!(ErasedAssetSettings);

impl<S: Settings> ErasedAssetSettings for AssetSettings<S> {
    fn id(&self) -> ErasedId {
        self.id
    }
}

#[allow(unused_imports)]
mod tests {
    use crate::{
        ext::{DeserializeExt, SerializeExt},
        settings::Settings,
    };
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
    struct TestSettings {
        value: u32,
    }

    impl Settings for TestSettings {}

    #[test]
    fn test_serialize_settings() {
        let mut settings = TestSettings::default();
        settings.value = 5;

        let bytes = settings.to_bytes().unwrap();
        let deserialized = TestSettings::from_bytes(&bytes).unwrap();

        assert_eq!(settings, deserialized);
    }
}
