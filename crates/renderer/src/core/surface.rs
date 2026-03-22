use crate::core::device::RenderDevice;
use ecs::{EventReader, Resource};
use wgpu::{
    Adapter, CompositeAlphaMode, CreateSurfaceError, Instance, PowerPreference, PresentMode,
    RequestAdapterOptions, Surface, SurfaceConfiguration, TextureFormat, TextureUsages,
    rwh::HandleError,
};
use window::{Window, events::WindowResized};

#[derive(Resource)]
pub struct RenderSurface {
    config: SurfaceConfiguration,
    inner: Surface<'static>,
    adapter: Adapter,
}

#[derive(Debug)]
pub enum RenderSurfaceError {
    SurfaceTarget(HandleError),
    CreateSurface(CreateSurfaceError),
}

impl From<HandleError> for RenderSurfaceError {
    fn from(value: HandleError) -> Self {
        Self::SurfaceTarget(value)
    }
}

impl From<CreateSurfaceError> for RenderSurfaceError {
    fn from(value: CreateSurfaceError) -> Self {
        Self::CreateSurface(value)
    }
}

impl RenderSurface {
    pub async fn new(window: &Window) -> Result<Self, RenderSurfaceError> {
        let instance = Instance::default();

        let surface = unsafe {
            let target = wgpu::SurfaceTargetUnsafe::from_window(window.inner())
                .map_err(RenderSurfaceError::from)?;
            instance
                .create_surface_unsafe(target)
                .map_err(RenderSurfaceError::from)?
        };

        let adapter = instance
            .request_adapter(&RequestAdapterOptions {
                power_preference: PowerPreference::HighPerformance,
                force_fallback_adapter: false,
                compatible_surface: Some(&surface),
            })
            .await
            .unwrap();

        let caps = surface.get_capabilities(&adapter);
        let format = caps
            .formats
            .first()
            .copied()
            .unwrap_or(TextureFormat::Bgra8Unorm);
        let present_mode = caps
            .present_modes
            .first()
            .copied()
            .unwrap_or(PresentMode::Fifo);

        let config = SurfaceConfiguration {
            usage: TextureUsages::RENDER_ATTACHMENT
                | TextureUsages::COPY_SRC
                | TextureUsages::COPY_DST,
            format,
            width: window.size().width,
            height: window.size().height,
            present_mode,
            desired_maximum_frame_latency: 2,
            alpha_mode: CompositeAlphaMode::Auto,
            view_formats: vec![format.add_srgb_suffix()],
        };

        Ok(Self {
            config,
            inner: surface,
            adapter,
        })
    }

    pub fn config(&self) -> &SurfaceConfiguration {
        &self.config
    }

    pub fn inner(&self) -> &Surface<'static> {
        &self.inner
    }

    pub fn adapter(&self) -> &Adapter {
        &self.adapter
    }

    pub fn width(&self) -> u32 {
        self.config.width
    }

    pub fn height(&self) -> u32 {
        self.config.height
    }

    pub fn format(&self) -> wgpu::TextureFormat {
        self.config.format
    }

    pub fn resize(&mut self, device: &RenderDevice, width: u32, height: u32) {
        self.config.width = width;
        self.config.height = height;
        self.inner.configure(device, &self.config);
    }

    pub fn configure(&mut self, device: &RenderDevice) {
        self.inner.configure(device, &self.config);
    }

    pub fn texture(&self) -> Result<wgpu::SurfaceTexture, wgpu::SurfaceError> {
        self.inner.get_current_texture()
    }

    pub(crate) fn on_resize(
        events: EventReader<WindowResized>,
        device: &RenderDevice,
        surface: &mut RenderSurface,
    ) {
        if let Some(event) = events.last() {
            surface.resize(device, event.width(), event.height());
        }
    }
}

#[derive(Default, Resource)]
pub struct RenderSurfaceTexture(Option<wgpu::SurfaceTexture>);

impl RenderSurfaceTexture {
    pub fn set(&mut self, texture: wgpu::SurfaceTexture) {
        self.0 = Some(texture);
    }

    pub fn get(&self) -> Option<&wgpu::SurfaceTexture> {
        self.0.as_ref()
    }

    pub(crate) fn present(surface: &mut RenderSurfaceTexture) {
        let Some(texture) = surface.0.take() else {
            return;
        };

        texture.present();
    }
}
