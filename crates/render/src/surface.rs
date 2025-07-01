use crate::device::RenderDevice;
use ecs::{Commands, Resource, app::Main, commands::AddResource};
use math::Size;
use wgpu::{PresentMode, SurfaceConfiguration, SurfaceTargetUnsafe, rwh::HandleError};
use window::{Window, app::WindowCommandsExt};

#[derive(Debug)]
pub enum RenderSurfaceError {
    Create(wgpu::CreateSurfaceError),
    Adapter(wgpu::RequestAdapterError),
    Device(wgpu::RequestDeviceError),
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
            Self::Device(e) => write!(f, "Failed to request device: {}", e),
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
    surface: wgpu::Surface<'static>,
    config: SurfaceConfiguration,
    depth_format: wgpu::TextureFormat,
}

impl RenderSurface {
    pub const DEFAULT_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;
    pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

    pub async fn new(window: &Window) -> Result<(Self, wgpu::Adapter), RenderSurfaceError> {
        let instance = wgpu::Instance::default();
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions::default())
            .await
            .unwrap();

        let size = window.size();

        let surface = unsafe {
            let target = SurfaceTargetUnsafe::from_window(window.inner())
                .map_err(|e| RenderSurfaceError::from(e))?;

            instance
                .create_surface_unsafe(target)
                .map_err(|e| RenderSurfaceError::from(e))?
        };
        let capabilities = surface.get_capabilities(&adapter);
        let format = capabilities.formats[0];
        let present_mode = match capabilities.present_modes.contains(&PresentMode::Mailbox) {
            true => PresentMode::Mailbox,
            false => PresentMode::Fifo,
        };
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width,
            height: size.height,
            present_mode,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: vec![format.add_srgb_suffix()],
            desired_maximum_frame_latency: 2,
        };

        let surface = Self {
            window: window.clone(),
            surface,
            config,
            depth_format: Self::DEPTH_FORMAT,
        };

        Ok((surface, adapter))
        // let instance = wgpu::Instance::default();

        // let surface = unsafe {
        //     let target = SurfaceTargetUnsafe::from_window(window.inner())
        //         .map_err(|e| RenderSurfaceError::from(e))?;

        //     instance
        //         .create_surface_unsafe(target)
        //         .map_err(|e| RenderSurfaceError::from(e))?
        // };

        // let size = window.size();

        // let adapter = instance
        //     .request_adapter(&wgpu::RequestAdapterOptions {
        //         power_preference: wgpu::PowerPreference::HighPerformance,
        //         compatible_surface: Some(&surface),
        //         ..Default::default()
        //     })
        //     .await
        //     .map_err(RenderSurfaceError::Adapter)?;

        // let capabilities = surface.get_capabilities(&adapter);

        // let format = *capabilities
        //     .formats
        //     .iter()
        //     .find(|format| **format == Self::DEFAULT_FORMAT)
        //     .unwrap_or(capabilities.formats.get(0).expect("No supported formats"));

        // let depth_format = Self::DEPTH_FORMAT;

        // let present_mode = match capabilities.present_modes.contains(&PresentMode::Mailbox) {
        //     true => PresentMode::Mailbox,
        //     false => PresentMode::Fifo,
        // };

        // let config = wgpu::SurfaceConfiguration {
        //     usage: capabilities.usages - wgpu::TextureUsages::STORAGE_BINDING,
        //     format,
        //     width: size.width,
        //     height: size.height,
        //     present_mode,
        //     alpha_mode: wgpu::CompositeAlphaMode::Auto,
        //     view_formats: vec![format.add_srgb_suffix()],
        //     desired_maximum_frame_latency: 2,
        // };

        // let surface = Self {
        //     window,
        //     surface: Arc::new(surface),
        //     config,
        //     depth_format,
        // };

        // Ok((surface, adapter))
    }

    pub fn surface(&self) -> &wgpu::Surface<'static> {
        &self.surface
    }

    pub fn window(&self) -> &Window {
        &self.window
    }

    pub fn size(&self) -> Size<u32> {
        Size {
            width: self.config.width,
            height: self.config.height,
        }
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
        let f = async {
            let (surface, adapter) = match RenderSurface::new(&window).await {
                Ok(value) => value,
                Err(error) => return Err(error),
            };

            let device = match RenderDevice::new(&adapter).await {
                Ok(device) => device,
                Err(error) => return Err(RenderSurfaceError::Device(error)),
            };

            surface.configure(&device);

            Ok((surface, device))
        };

        match smol::block_on(f) {
            Ok((surface, device)) => {
                commands.add(AddResource::from(surface));
                commands.add(AddResource::from(device));
            }
            Err(error) => main_commands.exit_error(error),
        };
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
            Ok(texture) if texture.suboptimal => {
                drop(texture);
                surface.configure(device);
            }
            Ok(texture) => surface_texture.set(texture),
            Err(wgpu::SurfaceError::Lost) | Err(wgpu::SurfaceError::Outdated) => {
                surface.configure(device);
            }
            _ => return,
        };
    }

    pub(crate) fn present_surface(surface_texture: &mut RenderSurfaceTexture) {
        surface_texture.present();
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

    fn present(&mut self) {
        if let Some(surface) = self.0.take() {
            surface.present();
        }
    }
}
