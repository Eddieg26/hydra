use crate::{
    core::RenderDevice,
    resources::extract::{ExtractError, ExtractInfo, ExtractedAsset, RenderAsset, RenderAssets},
};
use asset::{Asset, AssetId, Assets};
use ecs::{
    Command, Resource,
    unlifetime::{Read, SCommands},
};
use std::{
    borrow::Cow,
    collections::{HashMap, HashSet},
    fmt::Display,
};
use wgpu::{
    ShaderModuleDescriptor,
    naga::{
        WithSpan,
        front::wgsl::{self},
        valid::{Capabilities, ValidationError, ValidationFlags, Validator},
    },
};
use wgsl_macro::ShaderConstants;

#[derive(Debug, Clone, Asset)]
pub struct Shader {
    data: Cow<'static, str>,
    constants: ShaderConstants,
    validate: bool,
}

impl Shader {
    pub fn new(
        data: impl Into<Cow<'static, str>>,
        constants: ShaderConstants,
        validate: bool,
    ) -> Self {
        Self {
            data: data.into(),
            constants,
            validate,
        }
    }
}

#[derive(Debug)]
pub enum ShaderError {
    Processor(String),
    Parse(wgsl::ParseError),
    Validation(ValidationError),
}

impl From<wgsl_macro::ShaderProcessorError<'_>> for ShaderError {
    fn from(value: wgsl_macro::ShaderProcessorError) -> Self {
        Self::Processor(value.to_string())
    }
}

impl From<wgsl::ParseError> for ShaderError {
    fn from(value: wgsl::ParseError) -> Self {
        Self::Parse(value)
    }
}

impl From<WithSpan<ValidationError>> for ShaderError {
    fn from(value: WithSpan<ValidationError>) -> Self {
        Self::Validation(value.into_inner())
    }
}

impl Display for ShaderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ShaderError::Processor(e) => e.fmt(f),
            ShaderError::Parse(e) => e.fmt(f),
            ShaderError::Validation(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for ShaderError {}

#[derive(Clone)]
pub struct GpuShader(wgpu::ShaderModule);

impl GpuShader {
    pub fn new(device: &RenderDevice, shader: &Shader) -> Result<Self, ShaderError> {
        let mut processor = wgsl_macro::ShaderProcessor::new();
        let source = processor
            .build(&shader.data, &shader.constants)
            .map_err(ShaderError::from)?;

        if shader.validate {
            let mut validator = Validator::new(ValidationFlags::all(), Capabilities::all());
            let _ = validator
                .validate(&wgsl::parse_str(&source).map_err(ShaderError::from)?)
                .map_err(ShaderError::from)?;
        }

        let module = device.create_shader_module(ShaderModuleDescriptor {
            label: None,
            source: wgpu::ShaderSource::Wgsl(source.into()),
        });

        Ok(Self(module))
    }
}

impl std::ops::Deref for GpuShader {
    type Target = wgpu::ShaderModule;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl RenderAsset for GpuShader {
    type Asset = Shader;

    type Arg = (Read<RenderDevice>, SCommands);

    fn extract(
        _id: AssetId<Self::Asset>,
        asset: Self::Asset,
        (device, _): &mut ecs::ArgItem<Self::Arg>,
    ) -> Result<Self, ExtractError<Self::Asset>> {
        GpuShader::new(device, &asset).map_err(ExtractError::from_error)
    }

    fn removed(id: &AssetId<Self::Asset>, _: &Self, (_, commands): &mut ecs::ArgItem<Self::Arg>) {
        commands.add(RemoveShaderVariants(*id));
    }
}

pub struct ShaderVariant {
    pub id: AssetId<Shader>,
    pub constants: ShaderConstants,
}

#[derive(Resource)]
pub struct ShaderVariants {
    queue: HashMap<AssetId<Shader>, Vec<ShaderVariant>>,
    variants: HashMap<AssetId<Shader>, HashSet<AssetId<Shader>>>,
}

impl ShaderVariants {
    pub fn new() -> Self {
        Self {
            queue: HashMap::new(),
            variants: HashMap::new(),
        }
    }

    pub fn queue(&mut self, shader: AssetId<Shader>, variant: ShaderVariant) {
        self.queue.entry(shader).or_default().push(variant);
    }

    pub fn remove(&mut self, shader: &AssetId<Shader>) -> Option<HashSet<AssetId<Shader>>> {
        self.queue.remove(shader);
        self.variants.remove(shader)
    }

    pub(crate) fn extract(
        variants: &mut Self,
        assets: &Assets<Shader>,
        extract_info: &mut ExtractInfo<GpuShader>,
    ) {
        for (shader_id, variants) in variants.queue.drain() {
            let Some((data, validate)) =
                assets.get(&shader_id).map(|s| (s.data.clone(), s.validate))
            else {
                continue;
            };

            for variant in variants {
                extract_info.assets.push(ExtractedAsset {
                    id: variant.id,
                    asset: Shader::new(data.clone(), variant.constants, validate),
                });
            }
        }
    }
}

pub struct RemoveShaderVariants(pub AssetId<Shader>);
impl Command for RemoveShaderVariants {
    fn execute(self, world: &mut ecs::World) {
        let Some(variants) = world.resource_mut::<ShaderVariants>().remove(&self.0) else {
            return;
        };

        let shaders = world.resource_mut::<RenderAssets<GpuShader>>();
        for id in variants {
            shaders.remove(&id);
        }
    }
}
