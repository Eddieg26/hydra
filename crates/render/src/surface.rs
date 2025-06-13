use crate::device::RenderDevice;
use ecs::{Commands, Resource, app::Main, commands::AddResource};
use std::sync::Arc;
use wgpu::{PresentMode, SurfaceConfiguration, SurfaceTargetUnsafe, rwh::HandleError};
use window::{Window, app::WindowCommandsExt};

#[derive(Debug)]
pub enum RenderSurfaceError {
    Create(wgpu::CreateSurfaceError),
    Adapter(wgpu::RequestAdapterError),
    Handle(HandleError),
}

impl From<wgpu::CreateSurfaceError> for RenderSurfaceError {
    fn from(error: wgpu::CreateSurfaceError) -> Self {
        Self::Create(error)
    }
}

impl std::fmt::Display for RenderSurfaceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Create(e) => write!(f, "Failed to create surface: {}", e),
            Self::Adapter(e) => write!(f, "Failed to request adapter: {}", e),
            Self::Handle(e) => write!(f, "{}", e),
        }
    }
}

impl From<HandleError> for RenderSurfaceError {
    fn from(error: HandleError) -> Self {
        Self::Handle(error)
    }
}

impl std::error::Error for RenderSurfaceError {}

#[derive(Resource)]
pub struct RenderSurface {
    window: Window,
    surface: Arc<wgpu::Surface<'static>>,
    config: SurfaceConfiguration,
    depth_format: wgpu::TextureFormat,
}

impl RenderSurface {
    pub const DEFAULT_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;
    pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

    pub async fn new(window: Window) -> Result<(Self, wgpu::Adapter), RenderSurfaceError> {
        let instance = wgpu::Instance::default();

        let surface = unsafe {
            let target = SurfaceTargetUnsafe::from_window(window.inner())
                .map_err(|e| RenderSurfaceError::from(e))?;

            instance
                .create_surface_unsafe(target)
                .map_err(|e| RenderSurfaceError::from(e))?
        };

        let size = window.size();

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                ..Default::default()
            })
            .await
            .map_err(RenderSurfaceError::Adapter)?;

        let capabilities = surface.get_capabilities(&adapter);

        let format = *capabilities
            .formats
            .iter()
            .find(|format| **format == Self::DEFAULT_FORMAT)
            .unwrap_or(capabilities.formats.get(0).expect("No supported formats"));

        let depth_format = Self::DEPTH_FORMAT;

        let present_mode = capabilities
            .present_modes
            .iter()
            .find(|mode| **mode == PresentMode::Mailbox)
            .cloned()
            .unwrap_or_default();

        let alpha_mode = capabilities.alpha_modes[0];

        let config = wgpu::SurfaceConfiguration {
            usage: capabilities.usages - wgpu::TextureUsages::STORAGE_BINDING,
            format,
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: vec![format.add_srgb_suffix()],
            desired_maximum_frame_latency: 2,
        };

        let surface = Self {
            window,
            surface: Arc::new(surface),
            config,
            depth_format,
        };

        Ok((surface, adapter))
    }

    pub fn surface(&self) -> &wgpu::Surface<'static> {
        &self.surface
    }

    pub fn window(&self) -> &Window {
        &self.window
    }

    pub fn width(&self) -> u32 {
        self.config.width
    }

    pub fn height(&self) -> u32 {
        self.config.height
    }

    pub fn config(&self) -> &SurfaceConfiguration {
        &self.config
    }

    pub fn format(&self) -> wgpu::TextureFormat {
        self.config.format
    }

    pub fn depth_format(&self) -> wgpu::TextureFormat {
        self.depth_format
    }

    pub fn configure(&self, device: &RenderDevice) {
        self.surface.configure(device, &self.config);
    }

    pub fn texture(&self) -> Result<wgpu::SurfaceTexture, wgpu::SurfaceError> {
        self.surface.get_current_texture()
    }

    pub fn resize(&mut self, device: &RenderDevice, width: u32, height: u32) {
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(device, &self.config);
    }

    pub(crate) fn create_surface(
        window: Main<&Window>,
        mut commands: Commands,
        mut main_commands: Main<Commands>,
    ) {
        let (surface, adapter) = match smol::block_on(RenderSurface::new(window.clone())) {
            Ok(result) => result,
            Err(error) => return main_commands.exit_error(error),
        };

        let device = match smol::block_on(RenderDevice::new(&adapter)) {
            Ok(device) => device,
            Err(error) => return main_commands.exit_error(error),
        };

        surface.configure(&device);

        commands.add(AddResource::from(surface));
        commands.add(AddResource::from(device));
    }

    pub(crate) fn resize_surface(
        window: Main<&Window>,
        surface: &mut RenderSurface,
        device: &RenderDevice,
    ) {
        let size = window.size();
        if size.width != surface.width() || size.height != surface.height() {
            surface.resize(device, size.width, size.height);
        }
    }

    pub(crate) fn queue_surface(
        device: &RenderDevice,
        surface: &RenderSurface,
        surface_texture: &mut RenderSurfaceTexture,
    ) {
        match surface.texture() {
            Ok(texture) => {
                if texture.suboptimal {
                    surface.configure(device);
                }
                surface_texture.set(texture);
            }
            Err(wgpu::SurfaceError::Lost) | Err(wgpu::SurfaceError::Outdated) => {
                surface.configure(device);
            }
            _ => return,
        };
    }

    pub(crate) fn present_surface(
        surface: &RenderSurface,
        surface_texture: &mut RenderSurfaceTexture,
    ) {
        surface_texture.present(&surface.window);
    }
}

#[derive(Resource)]
pub struct RenderSurfaceTexture(Option<wgpu::SurfaceTexture>);

impl RenderSurfaceTexture {
    pub fn new() -> Self {
        Self(None)
    }

    pub fn set(&mut self, surface: wgpu::SurfaceTexture) {
        self.0 = Some(surface);
    }

    pub fn get(&self) -> Option<&wgpu::SurfaceTexture> {
        self.0.as_ref()
    }

    fn present(&mut self, window: &Window) {
        if let Some(surface) = self.0.take() {
            window.inner().pre_present_notify();
            surface.present();
        }
    }
}
