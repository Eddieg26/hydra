use crate::{core::RenderDevice, resources::extract::RenderAsset, types::Color};
use asset::Asset;
use ecs::{
    Resource,
    unlifetime::{Read, Write},
};
use std::{collections::HashMap, sync::Arc};
use wgpu::{
    CompareFunction, FilterMode, Label, Sampler, SamplerBorderColor, TextureFormat, TextureUsages,
    TextureView,
    util::DeviceExt,
    wgt::{TextureDataOrder, TextureDescriptor},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TextureDimension {
    D1 {
        size: u32,
    },
    D2 {
        width: u32,
        height: u32,
    },
    D2Array {
        width: u32,
        height: u32,
        layers: u32,
    },
    D3 {
        width: u32,
        height: u32,
        depth: u32,
    },
    Cube {
        size: u32,
    },
    CubeArray {
        size: u32,
        layers: u32,
    },
}

impl TextureDimension {
    pub fn extents(&self) -> wgpu::Extent3d {
        let (width, height, depth_or_array_layers) = match self {
            TextureDimension::D1 { size } => (*size, 1, 1),
            TextureDimension::D2 { width, height } => (*width, *height, 1),
            TextureDimension::D2Array {
                width,
                height,
                layers,
            } => (*width, *height, *layers),
            TextureDimension::D3 {
                width,
                height,
                depth,
            } => (*width, *height, *depth),
            TextureDimension::Cube { size } => (*size, *size, 6),
            TextureDimension::CubeArray { size, layers } => (*size, *size, *layers * 6),
        };

        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers,
        }
    }
}

impl Into<wgpu::TextureDimension> for TextureDimension {
    fn into(self) -> wgpu::TextureDimension {
        match self {
            TextureDimension::D1 { .. } => wgpu::TextureDimension::D1,
            TextureDimension::D2 { .. } => wgpu::TextureDimension::D2,
            TextureDimension::D2Array { .. } => wgpu::TextureDimension::D2,
            TextureDimension::D3 { .. } => wgpu::TextureDimension::D3,
            TextureDimension::Cube { .. } => wgpu::TextureDimension::D2,
            TextureDimension::CubeArray { .. } => wgpu::TextureDimension::D2,
        }
    }
}

impl Into<wgpu::TextureViewDimension> for TextureDimension {
    fn into(self) -> wgpu::TextureViewDimension {
        match self {
            TextureDimension::D1 { .. } => wgpu::TextureViewDimension::D1,
            TextureDimension::D2 { .. } => wgpu::TextureViewDimension::D2,
            TextureDimension::D2Array { .. } => wgpu::TextureViewDimension::D2Array,
            TextureDimension::D3 { .. } => wgpu::TextureViewDimension::D3,
            TextureDimension::Cube { .. } => wgpu::TextureViewDimension::Cube,
            TextureDimension::CubeArray { .. } => wgpu::TextureViewDimension::CubeArray,
        }
    }
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WrapMode {
    #[default]
    ClampToEdge,
    Repeat,
    MirrorRepeat,
    ClampToBorder(SamplerBorderColor),
}

impl WrapMode {
    pub fn border_color(&self) -> Option<SamplerBorderColor> {
        match self {
            WrapMode::ClampToBorder(color) => Some(*color),
            _ => None,
        }
    }
}

impl Into<wgpu::AddressMode> for WrapMode {
    fn into(self) -> wgpu::AddressMode {
        match self {
            WrapMode::Repeat => wgpu::AddressMode::Repeat,
            WrapMode::ClampToEdge => wgpu::AddressMode::ClampToEdge,
            WrapMode::ClampToBorder(_) => wgpu::AddressMode::ClampToBorder,
            WrapMode::MirrorRepeat => wgpu::AddressMode::MirrorRepeat,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TextureSampler {
    Default,
    Custom {
        filter: FilterMode,
        wrap: WrapMode,
        compare: Option<CompareFunction>,
        anisotropy: u16,
    },
}

#[derive(Debug, Clone, Asset)]
pub struct Texture {
    pub dimension: TextureDimension,
    pub format: TextureFormat,
    pub mips: bool,
    pub usage: TextureUsages,
    pub sampler: TextureSampler,
    pub pixels: Vec<u8>,
}

impl Texture {
    pub fn solid(color: Color, dimension: TextureDimension) -> Self {
        let (r, g, b, a) = color.to_bytes();
        let pixel: &[u8; 4] = &[r, g, b, a];
        let extent = dimension.extents();
        let num_pixels = extent.width * extent.height * extent.depth_or_array_layers;
        let mut pixels = Vec::new();

        for _ in 0..num_pixels {
            pixels.extend_from_slice(pixel);
        }

        Self {
            dimension,
            format: TextureFormat::Rgba8UnormSrgb,
            mips: false,
            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
            sampler: TextureSampler::Default,
            pixels,
        }
    }
}

pub struct GpuTexture {
    inner: Arc<wgpu::Texture>,
    view: TextureView,
    sampler: SamplerId,
}

impl GpuTexture {
    pub fn new(device: &RenderDevice, texture: &Texture, sampler: SamplerId) -> Self {
        let mip_level_count = match texture.mips {
            true => texture
                .dimension
                .extents()
                .max_mips(texture.dimension.into()) as u32,
            false => 1,
        };

        let texture = device.create_texture_with_data(
            device.queue(),
            &TextureDescriptor {
                label: None,
                size: texture.dimension.extents(),
                mip_level_count,
                sample_count: 1,
                dimension: texture.dimension.into(),
                format: texture.format,
                usage: texture.usage,
                view_formats: &[texture.format.add_srgb_suffix()],
            },
            TextureDataOrder::LayerMajor,
            &texture.pixels,
        );

        let view = texture.create_view(&Default::default());

        Self {
            inner: Arc::new(texture),
            view,
            sampler,
        }
    }

    pub fn inner(&self) -> &wgpu::Texture {
        &self.inner
    }

    pub fn view(&self) -> &TextureView {
        &self.view
    }

    pub fn sampler(&self) -> SamplerId {
        self.sampler
    }
}

impl RenderAsset for GpuTexture {
    type Asset = Texture;

    type Arg = (Read<RenderDevice>, Write<SamplerCache>);

    fn extract(
        _: asset::AssetId<Self::Asset>,
        asset: Self::Asset,
        (device, samplers): &mut ecs::ArgItem<Self::Arg>,
    ) -> Result<Self, super::extract::ExtractError<Self::Asset>> {
        let sampler = samplers.allocate(device, asset.sampler);
        let texture = GpuTexture::new(device, &asset, sampler);

        Ok(texture)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SamplerId(usize);

pub struct SamplerDesc<'a> {
    pub label: Label<'a>,
    pub wrap: WrapMode,
    pub filter: FilterMode,
    pub compare: Option<CompareFunction>,
    pub lod_min_clamp: f32,
    pub lod_max_clamp: f32,
    pub anisotropy_clamp: u16,
}

#[derive(Resource)]
pub struct SamplerCache {
    samplers: Vec<Sampler>,
    allocated: HashMap<TextureSampler, SamplerId>,
}

impl SamplerCache {
    pub fn new(default: Sampler) -> Self {
        Self {
            samplers: vec![default],
            allocated: HashMap::from_iter(std::iter::once((TextureSampler::Default, SamplerId(0)))),
        }
    }

    fn new_sampler(device: &RenderDevice, desc: &SamplerDesc) -> Sampler {
        let address_mode = desc.wrap.into();
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: desc.label,
            address_mode_u: address_mode,
            address_mode_v: address_mode,
            address_mode_w: address_mode,
            mag_filter: desc.filter,
            min_filter: desc.filter,
            mipmap_filter: desc.filter,
            lod_min_clamp: desc.lod_min_clamp,
            lod_max_clamp: desc.lod_max_clamp,
            compare: desc.compare,
            anisotropy_clamp: desc.anisotropy_clamp,
            border_color: desc.wrap.border_color(),
        });

        sampler
    }

    pub fn get(&self, id: SamplerId) -> &Sampler {
        &self.samplers[id.0]
    }

    pub fn allocate(&mut self, device: &RenderDevice, sampler: TextureSampler) -> SamplerId {
        if let Some(id) = self.allocated.get(&sampler) {
            return *id;
        } else {
            let id = self.samplers.len();
            let (filter, wrap, compare, anisotropy) = match sampler {
                TextureSampler::Default => todo!(),
                TextureSampler::Custom {
                    filter,
                    wrap,
                    compare,
                    anisotropy,
                } => (filter, wrap, compare, anisotropy),
            };

            let desc = SamplerDesc {
                label: None,
                wrap,
                filter,
                compare,
                anisotropy_clamp: anisotropy,
                lod_min_clamp: 0.0,
                lod_max_clamp: 32.0,
            };

            self.samplers.push(Self::new_sampler(device, &desc));
            self.allocated.insert(sampler, SamplerId(id));

            SamplerId(id)
        }
    }

    pub fn clear(&mut self) {
        self.samplers.clear();
        self.allocated.clear();
    }
}
