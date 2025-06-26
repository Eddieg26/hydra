use super::RenderAsset;
use crate::device::RenderDevice;
use asset::{
    Asset, AssetMetadata, DefaultSettings,
    importer::{AssetImporter, ImportContext},
    io::{AssetIoError, AsyncReader},
};
use ecs::system::{ArgItem, unlifetime::Read};
use smol::io::AsyncReadExt;
use std::{borrow::Cow, sync::Arc};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum ShaderStage {
    Vertex,
    Fragment,
    Compute,
}

impl Into<wgpu::naga::ShaderStage> for ShaderStage {
    fn into(self) -> wgpu::naga::ShaderStage {
        match self {
            Self::Vertex => wgpu::naga::ShaderStage::Vertex,
            Self::Fragment => wgpu::naga::ShaderStage::Fragment,
            Self::Compute => wgpu::naga::ShaderStage::Compute,
        }
    }
}

impl Into<wgpu::naga::ShaderStage> for &ShaderStage {
    fn into(self) -> wgpu::naga::ShaderStage {
        match self {
            ShaderStage::Vertex => wgpu::naga::ShaderStage::Vertex,
            ShaderStage::Fragment => wgpu::naga::ShaderStage::Fragment,
            ShaderStage::Compute => wgpu::naga::ShaderStage::Compute,
        }
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, Asset)]
pub enum ShaderSource {
    Spirv {
        data: Cow<'static, [u32]>,
    },
    Glsl {
        data: Cow<'static, str>,
        stage: ShaderStage,
    },
    Wgsl {
        data: Cow<'static, str>,
    },
}

#[derive(Asset)]
pub struct Shader {
    module: Arc<wgpu::ShaderModule>,
}
impl Shader {
    pub fn new(device: &RenderDevice, source: ShaderSource) -> Self {
        let module = match source {
            ShaderSource::Spirv { data } => {
                device.create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: None,
                    source: wgpu::ShaderSource::SpirV(Cow::Borrowed(&data)),
                })
            }
            ShaderSource::Glsl { data, stage } => {
                device.create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: None,
                    source: wgpu::ShaderSource::Glsl {
                        shader: data,
                        stage: stage.into(),
                        defines: Default::default(),
                    },
                })
            }
            ShaderSource::Wgsl { data } => {
                device.create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: None,
                    source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(&data)),
                })
            }
        };
        Self {
            module: Arc::new(module),
        }
    }
}

impl<'de> serde::Deserialize<'de> for Shader {
    fn deserialize<D>(_: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Err(serde::de::Error::custom("Deserialization not supported"))
    }
}

impl From<wgpu::ShaderModule> for Shader {
    fn from(shader: wgpu::ShaderModule) -> Self {
        Self {
            module: Arc::new(shader),
        }
    }
}

impl std::ops::Deref for Shader {
    type Target = wgpu::ShaderModule;
    fn deref(&self) -> &Self::Target {
        &self.module
    }
}

impl AsRef<wgpu::ShaderModule> for Shader {
    fn as_ref(&self) -> &wgpu::ShaderModule {
        &self.module
    }
}

impl RenderAsset for Shader {
    type Source = ShaderSource;

    type Arg = Read<RenderDevice>;

    fn extract(
        asset: Self::Source,
        device: &mut ArgItem<Self::Arg>,
    ) -> Result<Self, super::ExtractError<Self::Source>> {
        Ok(Shader::new(device, asset))
    }

    fn usage(_: &Self::Source) -> super::AssetUsage {
        super::AssetUsage::Discard
    }
}

#[derive(Debug)]
pub enum ShaderLoadError {
    Io(AssetIoError),
    InvalidExt(String),
    Parse(String),
}

impl From<wgpu::naga::front::wgsl::ParseError> for ShaderLoadError {
    fn from(err: wgpu::naga::front::wgsl::ParseError) -> Self {
        Self::Parse(err.to_string())
    }
}

impl From<wgpu::naga::front::spv::Error> for ShaderLoadError {
    fn from(err: wgpu::naga::front::spv::Error) -> Self {
        Self::Parse(err.to_string())
    }
}

impl From<wgpu::naga::front::glsl::Error> for ShaderLoadError {
    fn from(err: wgpu::naga::front::glsl::Error) -> Self {
        Self::Parse(err.to_string())
    }
}

impl From<AssetIoError> for ShaderLoadError {
    fn from(err: AssetIoError) -> Self {
        Self::Io(err)
    }
}

impl std::fmt::Display for ShaderLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(err) => write!(f, "IO error: {}", err),
            Self::InvalidExt(err) => write!(f, "Parse error: {}", err),
            Self::Parse(err) => write!(f, "WGSL parse error: {}", err),
        }
    }
}

impl From<std::io::Error> for ShaderLoadError {
    fn from(err: std::io::Error) -> Self {
        Self::Io(AssetIoError::from(err))
    }
}

impl std::error::Error for ShaderLoadError {}

impl AssetImporter for ShaderSource {
    type Asset = ShaderSource;

    type Settings = DefaultSettings;

    type Error = ShaderLoadError;

    async fn import(
        ctx: &mut ImportContext<'_>,
        reader: &mut dyn AsyncReader,
        _: &AssetMetadata<Self::Settings>,
    ) -> Result<Self::Asset, Self::Error> {
        use wgpu::naga::{front::*, valid::*};

        let ext = ctx.path().ext();

        match ext {
            Some("spv") => {
                let mut buffer = Vec::new();
                reader
                    .read_to_end(&mut buffer)
                    .await
                    .map_err(ShaderLoadError::from)?;

                let module =
                    spv::parse_u8_slice(&buffer, &wgpu::naga::front::spv::Options::default())
                        .map_err(ShaderLoadError::from)?;
                let mut validator = Validator::new(ValidationFlags::all(), Capabilities::all());
                validator
                    .validate(&module)
                    .map_err(|e| ShaderLoadError::Parse(e.to_string()))?;

                let data = Cow::Owned(buffer.iter().map(|b| *b as u32).collect());

                Ok(ShaderSource::Spirv { data })
            }
            Some("wgsl") => {
                let mut data = String::new();
                reader
                    .read_to_string(&mut data)
                    .await
                    .map_err(ShaderLoadError::from)?;

                let module = wgsl::parse_str(&data).map_err(ShaderLoadError::from)?;
                let mut validator = Validator::new(ValidationFlags::all(), Capabilities::all());
                validator
                    .validate(&module)
                    .map_err(|e| ShaderLoadError::Parse(e.to_string()))?;

                let data = Cow::Owned(data);

                Ok(ShaderSource::Wgsl { data })
            }
            Some("vert") => {
                let mut data = String::new();
                reader
                    .read_to_string(&mut data)
                    .await
                    .map_err(ShaderLoadError::from)?;
                Ok(ShaderSource::Glsl {
                    data: Cow::Owned(data),
                    stage: ShaderStage::Vertex,
                })
            }
            Some("frag") => {
                let mut data = String::new();
                reader
                    .read_to_string(&mut data)
                    .await
                    .map_err(ShaderLoadError::from)?;
                Ok(ShaderSource::Glsl {
                    data: Cow::Owned(data),
                    stage: ShaderStage::Fragment,
                })
            }
            Some("comp") => {
                let mut data = String::new();
                reader
                    .read_to_string(&mut data)
                    .await
                    .map_err(ShaderLoadError::from)?;
                Ok(ShaderSource::Glsl {
                    data: Cow::Owned(data),
                    stage: ShaderStage::Compute,
                })
            }
            _ => Err(ShaderLoadError::InvalidExt(format!(
                "Invalid extension: {:?}",
                ext
            ))),
        }
    }

    fn extensions() -> &'static [&'static str] {
        &["spv", "wgsl", "vert", "frag", "comp"]
    }
}
