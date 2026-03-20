use ecs::Resource;
use wgpu::TextureFormat;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Msaa {
    /// 1 sample
    Disabled,
    /// 2 samples
    Low,
    /// 4 samples
    Medium,
    /// 8 samples
    High,
}

impl Msaa {
    pub fn sample_count(&self) -> u32 {
        match self {
            Msaa::Disabled => 1,
            Msaa::Low => 2,
            Msaa::Medium => 4,
            Msaa::High => 8,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DepthFormat {
    D16,
    D24,
    D24Stencil8,
    D32,
    D32Stencil8,
}

impl From<DepthFormat> for TextureFormat {
    fn from(value: DepthFormat) -> Self {
        match value {
            DepthFormat::D16 => TextureFormat::Depth16Unorm,
            DepthFormat::D24 => TextureFormat::Depth24Plus,
            DepthFormat::D24Stencil8 => TextureFormat::Depth24PlusStencil8,
            DepthFormat::D32 => TextureFormat::Depth32Float,
            DepthFormat::D32Stencil8 => TextureFormat::Depth32FloatStencil8,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ColorFormat {
    Standard { srgb: bool },
    HDR,
}

impl From<ColorFormat> for TextureFormat {
    fn from(value: ColorFormat) -> Self {
        match value {
            ColorFormat::Standard { srgb: true } => TextureFormat::Bgra8UnormSrgb,
            ColorFormat::Standard { srgb: false } => TextureFormat::Bgra8Unorm,
            ColorFormat::HDR => TextureFormat::Rgba16Float,
        }
    }
}

#[derive(Debug, Clone, Copy, Resource)]
pub struct RenderSettings {
    msaa: Msaa,
    color: ColorFormat,
    depth: DepthFormat,
}

impl RenderSettings {
    pub fn new(msaa: Msaa, color: ColorFormat, depth: DepthFormat) -> Self {
        Self { msaa, color, depth }
    }

    pub fn msaa(&self) -> Msaa {
        self.msaa
    }

    pub fn srgb(&self) -> bool {
        matches!(self.color, ColorFormat::Standard { srgb: true })
    }

    pub fn hdr(&self) -> bool {
        matches!(self.color, ColorFormat::HDR)
    }

    pub fn color(&self) -> ColorFormat {
        self.color
    }

    pub fn depth(&self) -> DepthFormat {
        self.depth
    }
}

impl Default for RenderSettings {
    fn default() -> Self {
        Self {
            color: ColorFormat::Standard { srgb: false },
            msaa: Msaa::Disabled,
            depth: DepthFormat::D32,
        }
    }
}
