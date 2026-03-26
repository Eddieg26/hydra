use crate::{
    core::{ColorFormat, DepthFormat, Msaa, RenderSettings},
    renderer::{
        camera::SettingState,
        graph::{GraphResource, Name, ResourceKind, ResourceResolver},
    },
    resources::{MainRenderTarget, RenderAssets, RenderTarget},
};
use wgpu::TextureDescriptor;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SurfaceKind {
    Color(ColorFormat),
    Depth(DepthFormat),
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SurfaceSize {
    Auto,
    Fixed { width: u32, height: u32 },
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct SurfaceDesc {
    pub size: SurfaceSize,
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
            SurfaceSize::Auto => match resolver.camera() {
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
            SurfaceSize::Fixed { width, height } => (width, height, desc.msaa),
        };

        resolver.add_texture(width, height, 1);

        SurfaceDesc {
            size: SurfaceSize::Fixed { width, height },
            kind,
            msaa,
        }
    }

    fn create(
        device: &crate::core::RenderDevice,
        name: Name,
        desc: &Self::Desc,
    ) -> Self {
        let SurfaceSize::Fixed { width, height } = desc.size else {
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
        desc_a == desc_b
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
