use super::{GpuTexture, Sampler, SamplerDesc, Texture, TextureDimension};
use crate::{device::RenderDevice, resources::extract::RenderResource};
use ecs::{
    Resource,
    system::{ArgItem, unlifetime::Read},
};

pub struct Fallbacks {
    pub d1: GpuTexture,
    pub d2_array: GpuTexture,
    pub d2: GpuTexture,
    pub d3: GpuTexture,
    pub cube: GpuTexture,
    pub cube_array: GpuTexture,
    pub sampler: Sampler,
}

impl Fallbacks {
    pub fn new(device: &RenderDevice) -> Self {
        let sampler = Sampler::new(
            device,
            &SamplerDesc {
                label: Some("fallback_sampler".into()),
                ..Default::default()
            },
        );

        let d1 = GpuTexture::create(
            device,
            &Texture::default_white(TextureDimension::D1),
            sampler.clone(),
        );
        let d2 = GpuTexture::create(
            device,
            &Texture::default_white(TextureDimension::D2),
            sampler.clone(),
        );
        let d2_array = GpuTexture::create(
            device,
            &Texture::default_white(TextureDimension::D2Array),
            sampler.clone(),
        );
        let d3 = GpuTexture::create(
            device,
            &Texture::default_white(TextureDimension::D3),
            sampler.clone(),
        );
        let cube = GpuTexture::create(
            device,
            &Texture::default_white(TextureDimension::Cube),
            sampler.clone(),
        );
        let cube_array = GpuTexture::create(
            device,
            &Texture::default_white(TextureDimension::CubeArray),
            sampler.clone(),
        );

        Self {
            d1,
            d2_array,
            d2,
            d3,
            cube,
            cube_array,
            sampler,
        }
    }

    pub fn texture(&self, dimension: TextureDimension) -> &GpuTexture {
        match dimension {
            TextureDimension::D1 => &self.d1,
            TextureDimension::D2 => &self.d2,
            TextureDimension::D3 => &self.d3,
            TextureDimension::Cube => &self.cube,
            TextureDimension::D2Array => &self.d2_array,
            TextureDimension::CubeArray => &self.cube_array,
        }
    }
}

impl Resource for Fallbacks {}

impl RenderResource for Fallbacks {
    type Arg = Read<RenderDevice>;

    fn extract(arg: ArgItem<Self::Arg>) -> Result<Self, crate::resources::extract::ExtractError> {
        Ok(Fallbacks::new(&arg))
    }
}
