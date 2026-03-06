use crate::core::RenderDevice;
use asset::{Asset, AssetId, Assets};
use std::{
    borrow::Cow,
    collections::{HashMap, HashSet},
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

pub enum ShaderError<'a> {
    Processor(wgsl_macro::ShaderProcessorError<'a>),
    Parse(wgsl::ParseError),
    Validation(ValidationError),
}

impl<'a> From<wgsl_macro::ShaderProcessorError<'a>> for ShaderError<'a> {
    fn from(value: wgsl_macro::ShaderProcessorError<'a>) -> Self {
        Self::Processor(value)
    }
}

impl<'a> From<wgsl::ParseError> for ShaderError<'a> {
    fn from(value: wgsl::ParseError) -> Self {
        Self::Parse(value)
    }
}

impl<'a> From<WithSpan<ValidationError>> for ShaderError<'a> {
    fn from(value: WithSpan<ValidationError>) -> Self {
        Self::Validation(value.into_inner())
    }
}

#[derive(Clone)]
pub struct GpuShader(wgpu::ShaderModule);

impl GpuShader {
    pub fn new<'a>(device: &'a RenderDevice, shader: &'a Shader) -> Result<Self, ShaderError<'a>> {
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

pub struct ShaderVariant {
    pub id: AssetId<Shader>,
    pub constants: ShaderConstants,
}

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

    pub fn remove(&mut self, shader: &AssetId<Shader>) {
        self.variants.remove(shader);
        self.queue.remove(shader);
    }

    pub fn extract(&mut self, assets: &mut Assets<Shader>) {
        for (shader_id, variants) in self.queue.drain() {
            let Some((data, validate)) =
                assets.get(&shader_id).map(|s| (s.data.clone(), s.validate))
            else {
                continue;
            };

            for variant in variants {
                let shader = Shader::new(data.clone(), variant.constants, validate);
                assets.insert(variant.id, shader);
            }
        }
    }
}
