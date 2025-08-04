use super::RenderAsset;
use crate::device::RenderDevice;
use asset::{
    Asset, AssetId, AssetProcessor, AssetSettings, Settings,
    ext::PathExt,
    importer::{AssetImporter, ImportContext},
    io::{AssetPath, AsyncIoError, AsyncReader},
};
use ecs::{
    Resource,
    system::{ArgItem, unlifetime::Read},
};
use processor::{ShaderConstant, ShaderConstants, ShaderProcessor};
use smol::io::AsyncReadExt;
use std::{borrow::Cow, collections::HashMap, sync::Arc};

pub mod processor;

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

pub trait GlobalShaderConstant: 'static {
    const NAME: &'static str;

    // Returns the shader constant based on the given device.
    // This is used to retrieve the constant from the global shader constants resource.
    fn get(device: &RenderDevice) -> ShaderConstant;
}

#[derive(Resource, Default)]
pub struct GlobalShaderConstants {
    constants: ShaderConstants,
    local: HashMap<AssetId<Shader>, ShaderConstants>,
    registry: HashMap<&'static str, fn(&RenderDevice) -> ShaderConstant>,
}

impl GlobalShaderConstants {
    pub fn new() -> Self {
        Self {
            constants: ShaderConstants::new(),
            local: HashMap::new(),
            registry: HashMap::new(),
        }
    }

    pub fn register<C: GlobalShaderConstant>(&mut self) {
        self.registry
            .entry(C::NAME)
            .or_insert(|device| C::get(device));
    }

    pub fn local(&self, shader: &AssetId<Shader>) -> Option<&ShaderConstants> {
        self.local.get(shader)
    }

    pub fn add_local(&mut self, shader: AssetId<Shader>, constants: ShaderConstants) {
        self.local.insert(shader, constants);
    }

    pub(crate) fn init(constants: &mut Self, device: &RenderDevice) {
        let GlobalShaderConstants {
            constants,
            registry,
            ..
        } = constants;

        for (name, f) in registry.drain() {
            let constant = f(&device);
            constants.set(name.to_string(), constant);
        }
    }
}

impl std::ops::Deref for GlobalShaderConstants {
    type Target = ShaderConstants;

    fn deref(&self) -> &Self::Target {
        &self.constants
    }
}

impl std::ops::DerefMut for GlobalShaderConstants {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.constants
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Asset)]
pub enum Shader {
    Spirv {
        data: Cow<'static, [u32]>,
    },
    Glsl {
        data: Cow<'static, str>,
        stage: ShaderStage,
    },
    Wgsl {
        data: Cow<'static, str>,
        dependencies: HashMap<String, String>,
        constants: ShaderConstants,
    },
}

#[derive(Clone, Debug, thiserror::Error)]
#[error("Shader error: {0}")]
pub struct ShaderError(String);

#[derive(Asset)]
pub struct GpuShader {
    module: Arc<wgpu::ShaderModule>,
}
impl GpuShader {
    pub fn new(
        device: &RenderDevice,
        source: Shader,
        globals: &ShaderConstants,
        local: Option<&ShaderConstants>,
    ) -> Result<Self, ShaderError> {
        let module = match source {
            Shader::Spirv { data } => device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: None,
                source: wgpu::ShaderSource::SpirV(Cow::Borrowed(&data)),
            }),
            Shader::Glsl { data, stage } => {
                device.create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: None,
                    source: wgpu::ShaderSource::Glsl {
                        shader: data,
                        stage: stage.into(),
                        defines: Default::default(),
                    },
                })
            }
            Shader::Wgsl {
                data,
                dependencies,
                mut constants,
            } => {
                let mut processor = ShaderProcessor::new();
                for (path, module) in &dependencies {
                    processor.add_module(&path, &module);
                }

                for (name, constant) in globals.iter() {
                    if !constants.contains(name) {
                        constants.set(name.clone(), constant.clone());
                    }
                }

                if let Some(local) = local {
                    for (name, constant) in local.iter() {
                        constants.set(name.clone(), constant.clone());
                    }
                }

                let data = processor
                    .build(&data, &constants)
                    .map_err(|e| ShaderError(e.to_string()))?;

                device.create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: None,
                    source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(&data)),
                })
            }
        };

        Ok(Self {
            module: Arc::new(module),
        })
    }

    pub fn generate_padded_struct(name: &str, size_in_bytes: usize) -> String {
        let padded_size = (size_in_bytes + 15) & !15; // round up to nearest multiple of 16
        let full_chunks = padded_size / 16;

        let mut struct_def = format!("struct {name} {{\n",);

        for i in 0..full_chunks {
            struct_def.push_str(&format!("    data{}: vec4<u32>;\n", i));
        }

        struct_def.push_str("};");
        struct_def
    }
}

impl<'de> serde::Deserialize<'de> for GpuShader {
    fn deserialize<D>(_: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Err(serde::de::Error::custom("Deserialization not supported"))
    }
}

impl From<wgpu::ShaderModule> for GpuShader {
    fn from(shader: wgpu::ShaderModule) -> Self {
        Self {
            module: Arc::new(shader),
        }
    }
}

impl std::ops::Deref for GpuShader {
    type Target = wgpu::ShaderModule;
    fn deref(&self) -> &Self::Target {
        &self.module
    }
}

impl AsRef<wgpu::ShaderModule> for GpuShader {
    fn as_ref(&self) -> &wgpu::ShaderModule {
        &self.module
    }
}

impl RenderAsset for GpuShader {
    type Source = Shader;

    type Arg = (Read<RenderDevice>, Read<GlobalShaderConstants>);

    fn extract(
        id: AssetId<Self::Source>,
        asset: Self::Source,
        (device, global_constants): &mut ArgItem<Self::Arg>,
    ) -> Result<Self, super::ExtractError<Self::Source>> {
        let local = global_constants.local(&id);
        GpuShader::new(device, asset, &global_constants, local)
            .map_err(|e| super::ExtractError::from_error(e))
    }

    fn usage(_: &Self::Source) -> super::AssetUsage {
        super::AssetUsage::Discard
    }
}

#[derive(Debug)]
pub enum ShaderImportError {
    Io(AsyncIoError),
    InvalidExt(String),
    Parse(String),
    Processor(String),
}

impl From<wgpu::naga::front::wgsl::ParseError> for ShaderImportError {
    fn from(err: wgpu::naga::front::wgsl::ParseError) -> Self {
        Self::Parse(err.to_string())
    }
}

impl From<wgpu::naga::front::spv::Error> for ShaderImportError {
    fn from(err: wgpu::naga::front::spv::Error) -> Self {
        Self::Parse(err.to_string())
    }
}

impl From<wgpu::naga::front::glsl::Error> for ShaderImportError {
    fn from(err: wgpu::naga::front::glsl::Error) -> Self {
        Self::Parse(err.to_string())
    }
}

impl From<AsyncIoError> for ShaderImportError {
    fn from(err: AsyncIoError) -> Self {
        Self::Io(err)
    }
}

impl std::fmt::Display for ShaderImportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(err) => write!(f, "IO error: {}", err),
            Self::InvalidExt(err) => write!(f, "Parse error: {}", err),
            Self::Parse(err) => write!(f, "WGSL parse error: {}", err),
            Self::Processor(err) => write!(f, "Shader processor error: {}", err),
        }
    }
}

impl From<std::io::Error> for ShaderImportError {
    fn from(err: std::io::Error) -> Self {
        Self::Io(AsyncIoError::from(err))
    }
}

impl std::error::Error for ShaderImportError {}

#[derive(Default, Settings, Clone, serde::Serialize, serde::Deserialize)]
pub struct ShaderSettings {
    constants: ShaderConstants,
}

impl From<ShaderConstants> for ShaderSettings {
    fn from(constants: ShaderConstants) -> Self {
        Self { constants }
    }
}

impl AssetImporter for Shader {
    type Asset = Shader;

    type Settings = ShaderSettings;

    type Error = ShaderImportError;

    async fn import(
        ctx: &mut ImportContext<'_>,
        reader: &mut dyn AsyncReader,
        settings: &AssetSettings<Self::Settings>,
    ) -> Result<Self::Asset, Self::Error> {
        use wgpu::naga::{front::*, valid::*};

        let ext = ctx.path().ext();

        match ext {
            Some("spv") => {
                let mut buffer = Vec::new();
                reader
                    .read_to_end(&mut buffer)
                    .await
                    .map_err(ShaderImportError::from)?;

                let module =
                    spv::parse_u8_slice(&buffer, &wgpu::naga::front::spv::Options::default())
                        .map_err(ShaderImportError::from)?;
                let mut validator = Validator::new(ValidationFlags::all(), Capabilities::all());
                validator
                    .validate(&module)
                    .map_err(|e| ShaderImportError::Parse(e.to_string()))?;

                let data = Cow::Owned(buffer.iter().map(|b| *b as u32).collect());

                Ok(Shader::Spirv { data })
            }
            Some("wgsl") => {
                let mut data = String::new();
                reader
                    .read_to_string(&mut data)
                    .await
                    .map_err(ShaderImportError::from)?;

                // let module = wgsl::parse_str(&data).map_err(ShaderImportError::from)?;
                // let mut validator = Validator::new(ValidationFlags::all(), Capabilities::all());
                // validator
                //     .validate(&module)
                //     .map_err(|e| ShaderImportError::Parse(e.to_string()))?;

                let data = Cow::Owned(data);

                Ok(Shader::Wgsl {
                    data,
                    dependencies: HashMap::new(),
                    constants: settings.constants.clone(),
                })
            }
            Some("vert") => {
                let mut data = String::new();
                reader
                    .read_to_string(&mut data)
                    .await
                    .map_err(ShaderImportError::from)?;
                Ok(Shader::Glsl {
                    data: Cow::Owned(data),
                    stage: ShaderStage::Vertex,
                })
            }
            Some("frag") => {
                let mut data = String::new();
                reader
                    .read_to_string(&mut data)
                    .await
                    .map_err(ShaderImportError::from)?;
                Ok(Shader::Glsl {
                    data: Cow::Owned(data),
                    stage: ShaderStage::Fragment,
                })
            }
            Some("comp") => {
                let mut data = String::new();
                reader
                    .read_to_string(&mut data)
                    .await
                    .map_err(ShaderImportError::from)?;
                Ok(Shader::Glsl {
                    data: Cow::Owned(data),
                    stage: ShaderStage::Compute,
                })
            }
            _ => Err(ShaderImportError::InvalidExt(format!(
                "Invalid extension: {:?}",
                ext
            ))),
        }
    }

    fn extensions() -> &'static [&'static str] {
        &["spv", "wgsl", "vert", "frag", "comp"]
    }
}

impl AssetProcessor for Shader {
    type Input = Shader;

    type Output = Shader;

    type Error = ShaderImportError;

    async fn process(
        ctx: &mut asset::ProcessContext<'_>,
        mut asset: Self::Input,
    ) -> Result<Self::Output, Self::Error> {
        if let Shader::Wgsl {
            data, dependencies, ..
        } = &mut asset
        {
            let ctx = Arc::new(smol::lock::RwLock::new(ctx));

            let imports = ShaderProcessor::get_imports(&data, &ctx, |path, ctx| async move {
                let mut ctx = ctx.write().await;
                let path = AssetPath::from(path);
                let shader = ctx.load::<Shader>(path.clone()).await?;

                match &shader.asset {
                    Shader::Wgsl { data, .. } => Ok(data.to_string()),
                    _ => return Err(AsyncIoError::invalid_data()),
                }
            })
            .await
            .map_err(|e| ShaderImportError::Processor(e.to_string()))?;

            *dependencies = imports;
        }

        return Ok(asset);
    }
}
