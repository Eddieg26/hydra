use crate::{
    core::{ColorFormat, DepthFormat, Msaa, RenderSettings},
    renderer::{
        camera::SettingState,
        graph::{GraphResource, Name, ResourceKind, ResourceResolver},
    },
    resources::{MainRenderTarget, RenderAssets, RenderTarget, TextureSampler},
};
use wgpu::{
    Sampler, TextureDescriptor, TextureFormat, TextureUsages, TextureView, TextureViewDimension,
};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SurfaceKind {
    Color(ColorFormat),
    Depth(DepthFormat),
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum TextureSize {
    Auto,
    Texture {
        id: u32,
    },
    Fixed {
        width: u32,
        height: u32,
        depth_or_layers: u32,
    },
}

impl TextureSize {
    pub fn compatible(&self, size: TextureSize) -> bool {
        match (self, size) {
            (TextureSize::Auto, TextureSize::Auto) => true,
            (
                TextureSize::Fixed {
                    width: w1,
                    height: h1,
                    ..
                },
                TextureSize::Fixed {
                    width: w2,
                    height: h2,
                    ..
                },
            ) => w1 * h1 > w2 * h2,
            (TextureSize::Texture { id: i1 }, TextureSize::Texture { id: i2 }) => i1 == &i2,
            _ => false,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct SurfaceDesc {
    pub size: TextureSize,
    pub kind: SurfaceKind,
    pub msaa: Msaa,
}

pub struct SurfaceTexture {
    pub view: wgpu::TextureView,
    pub msaa_view: Option<wgpu::TextureView>,
}

impl GraphResource for SurfaceTexture {
    type Desc = SurfaceDesc;

    fn resolve(
        world: &ecs::World,
        settings: &crate::core::RenderSettings,
        resolver: &mut ResourceResolver,
        desc: Self::Desc,
    ) -> Self::Desc {
        let kind = match desc.kind {
            SurfaceKind::Color(_) => SurfaceKind::Color(settings.color()),
            SurfaceKind::Depth(_) => SurfaceKind::Depth(settings.depth()),
        };

        let (width, height, msaa) = match desc.size {
            TextureSize::Auto => match resolver.camera() {
                Some(camera) => {
                    let msaa = match camera.msaa {
                        SettingState::Auto => settings.msaa(),
                        SettingState::Disabled => Msaa::Disabled,
                    };

                    (camera.width, camera.height, msaa)
                }
                None => {
                    let main = world
                        .resource::<MainRenderTarget>()
                        .get()
                        .expect("Main Render Target not set");

                    (main.width, main.height, settings.msaa())
                }
            },
            TextureSize::Texture { id } => {
                let size = resolver.texture(id).expect("Texture not found");
                (size.width, size.height, settings.msaa())
            }
            TextureSize::Fixed { width, height, .. } => (width, height, desc.msaa),
        };

        resolver.add_texture(width, height, 1);

        SurfaceDesc {
            size: TextureSize::Fixed {
                width,
                height,
                depth_or_layers: 1,
            },
            kind,
            msaa,
        }
    }

    fn create(device: &crate::core::RenderDevice, name: Name, desc: &Self::Desc) -> Self {
        let TextureSize::Fixed { width, height, .. } = desc.size else {
            panic!("Suface Input Desc not resolved.")
        };

        let format = match desc.kind {
            SurfaceKind::Color(format) => wgpu::TextureFormat::from(format),
            SurfaceKind::Depth(format) => wgpu::TextureFormat::from(format),
        };

        let size = wgpu::Extent3d {
            width: width,
            height: height,
            depth_or_array_layers: 1,
        };

        let texture = device.create_texture(&TextureDescriptor {
            label: Some(name),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::all(),
            view_formats: &[],
        });

        let msaa = match desc.msaa {
            crate::core::Msaa::Disabled => None,
            _ => Some(device.create_texture(&TextureDescriptor {
                label: Some(&format!("{name}_MSAA")),
                size,
                mip_level_count: 1,
                sample_count: desc.msaa.sample_count(),
                dimension: wgpu::TextureDimension::D2,
                format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            })),
        };

        Self {
            view: texture.create_view(&Default::default()),
            msaa_view: msaa.map(|msaa| msaa.create_view(&Default::default())),
        }
    }

    fn entry(
        _: &RenderSettings,
        _: &Self::Desc,
        builder: &mut crate::resources::BindGroupLayoutBuilder,
        visibility: wgpu::ShaderStages,
    ) {
        builder.with_texture(
            visibility,
            wgpu::TextureSampleType::Float { filterable: true },
            wgpu::TextureViewDimension::D2,
            false,
            None,
        );
    }

    fn bind<'a>(&'a self, builder: &mut crate::resources::BindGroupBuilder<'a>) {
        builder.with_texture(&self.view);
    }

    fn compatible(desc_a: &Self::Desc, desc_b: &Self::Desc) -> bool {
        desc_a.size.compatible(desc_b.size)
            && desc_a.kind == desc_b.kind
            && desc_a.msaa == desc_b.msaa
    }

    fn generation(&self) -> u32 {
        0
    }

    fn kind() -> ResourceKind {
        ResourceKind::Transient
    }
}

pub struct RenderOutput {
    pub view: wgpu::TextureView,
    generation: u32,
}

#[derive(Clone, PartialEq, Eq)]
pub enum RenderOutputDesc {
    Auto,
    Resolved {
        view: wgpu::TextureView,
        generation: u32,
    },
}

impl GraphResource for RenderOutput {
    type Desc = RenderOutputDesc;

    const ROOT: bool = true;

    fn resolve(
        world: &ecs::World,
        _: &crate::core::RenderSettings,
        resolver: &mut ResourceResolver,
        desc: Self::Desc,
    ) -> Self::Desc {
        if let RenderOutputDesc::Resolved { .. } = &desc {
            return desc;
        };

        let targets = world.resource::<RenderAssets<RenderTarget>>();
        let target = match resolver.camera() {
            Some(camera) => camera.target.and_then(|id| targets.get(&id)),
            None => world.resource::<MainRenderTarget>().get(),
        };

        let Some(target) = target else {
            panic!("Render target not found.")
        };

        let view = target.color.view().clone();
        let generation = resolver.camera().map(|c| c.generation).unwrap_or(0);

        RenderOutputDesc::Resolved { view, generation }
    }

    fn create(_: &crate::core::RenderDevice, _: Name, desc: &Self::Desc) -> Self {
        let RenderOutputDesc::Resolved { view, generation } = desc else {
            panic!("Render output not resolved.")
        };

        Self {
            view: view.clone(),
            generation: *generation,
        }
    }

    fn entry(
        _: &RenderSettings,
        _: &Self::Desc,
        builder: &mut crate::resources::BindGroupLayoutBuilder,
        visibility: wgpu::ShaderStages,
    ) {
        builder.with_texture(
            visibility,
            wgpu::TextureSampleType::Float { filterable: true },
            wgpu::TextureViewDimension::D2,
            false,
            None,
        );
    }

    fn bind<'a>(&'a self, builder: &mut crate::resources::BindGroupBuilder<'a>) {
        builder.with_texture(&self.view);
    }

    fn compatible(desc_a: &Self::Desc, desc_b: &Self::Desc) -> bool {
        desc_a == desc_b
    }

    fn generation(&self) -> u32 {
        self.generation
    }

    fn kind() -> ResourceKind {
        ResourceKind::Imported
    }
}

impl GraphResource for Sampler {
    type Desc = TextureSampler;

    fn resolve(
        _: &ecs::World,
        _: &crate::core::RenderSettings,
        _: &mut ResourceResolver,
        desc: Self::Desc,
    ) -> Self::Desc {
        desc
    }

    fn create(device: &crate::core::RenderDevice, name: Name, desc: &Self::Desc) -> Self {
        device.create_sampler(&desc.desc(Some(name)).into())
    }

    fn entry(
        _: &RenderSettings,
        desc: &Self::Desc,
        builder: &mut crate::resources::BindGroupLayoutBuilder,
        visibility: wgpu::ShaderStages,
    ) {
        let ty = match desc {
            TextureSampler::Default => wgpu::SamplerBindingType::NonFiltering,
            TextureSampler::Custom {
                filter, compare, ..
            } => match (compare, filter) {
                (Some(_), _) => wgpu::SamplerBindingType::Comparison,
                (None, wgpu::FilterMode::Nearest) => wgpu::SamplerBindingType::NonFiltering,
                (None, wgpu::FilterMode::Linear) => wgpu::SamplerBindingType::Filtering,
            },
        };

        builder.with_sampler(visibility, ty, None);
    }

    fn bind<'a>(&'a self, builder: &mut crate::resources::BindGroupBuilder<'a>) {
        builder.with_sampler(self);
    }

    fn compatible(desc_a: &Self::Desc, desc_b: &Self::Desc) -> bool {
        desc_a == desc_b
    }

    fn generation(&self) -> u32 {
        0
    }

    fn kind() -> ResourceKind {
        ResourceKind::Imported
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct TextureDesc {
    pub size: TextureSize,
    pub format: TextureFormat,
    pub usage: TextureUsages,
    pub dimension: TextureViewDimension,
}

impl GraphResource for TextureView {
    type Desc = TextureDesc;

    fn resolve(
        world: &ecs::World,
        _: &crate::core::RenderSettings,
        resolver: &mut ResourceResolver,
        desc: Self::Desc,
    ) -> Self::Desc {
        let (width, height, depth_or_layers) = match desc.size {
            TextureSize::Auto => match resolver.camera() {
                Some(camera) => (camera.width, camera.height, 1),
                None => {
                    let main = world
                        .resource::<MainRenderTarget>()
                        .get()
                        .expect("Main Render Target not set");

                    (main.width, main.height, 1)
                }
            },
            TextureSize::Texture { id } => {
                let size = resolver.texture(id).expect("Texture not found");
                (size.width, size.height, size.depth_or_array_layers)
            }
            TextureSize::Fixed {
                width,
                height,
                depth_or_layers,
            } => (width, height, depth_or_layers),
        };

        TextureDesc {
            size: TextureSize::Fixed {
                width,
                height,
                depth_or_layers,
            },
            format: desc.format,
            usage: desc.usage,
            dimension: desc.dimension,
        }
    }

    fn create(device: &crate::core::RenderDevice, name: Name, desc: &Self::Desc) -> Self {
        let TextureSize::Fixed {
            width,
            height,
            depth_or_layers,
        } = desc.size
        else {
            panic!("Texture Size not resolved")
        };

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(name),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: depth_or_layers,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: desc.dimension.compatible_texture_dimension(),
            format: desc.format,
            usage: desc.usage,
            view_formats: &[],
        });

        texture.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(desc.dimension),
            ..Default::default()
        })
    }

    fn entry(
        _: &RenderSettings,
        desc: &Self::Desc,
        builder: &mut crate::resources::BindGroupLayoutBuilder,
        visibility: wgpu::ShaderStages,
    ) {
        let sample_ty = desc
            .format
            .sample_type(None, None)
            .expect(&format!("Invalid texture format {:?}", desc.format));

        builder.with_texture(visibility, sample_ty, desc.dimension, false, None);
    }

    fn bind<'a>(&'a self, builder: &mut crate::resources::BindGroupBuilder<'a>) {
        builder.with_texture(self);
    }

    fn compatible(desc_a: &Self::Desc, desc_b: &Self::Desc) -> bool {
        desc_a.size.compatible(desc_b.size)
            && desc_a.format == desc_b.format
            && desc_a.dimension == desc_b.dimension
            && desc_a.usage.contains(desc_b.usage)
    }

    fn generation(&self) -> u32 {
        0
    }

    fn kind() -> ResourceKind {
        ResourceKind::Imported
    }
}
