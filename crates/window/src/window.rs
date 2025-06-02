use ecs::Resource;
use std::sync::Arc;
use winit::{event_loop::ActiveEventLoop, window::WindowId};

#[derive(Resource)]
pub struct WindowConfig {
    pub width: u32,
    pub height: u32,
    pub title: String,
    pub resizable: bool,
    pub visible: bool,
    pub blur: bool,
    pub transparent: bool,
    pub maximized: bool,
    pub decorations: bool,
}

impl WindowConfig {
    pub fn new(title: impl ToString) -> Self {
        Self {
            width: 800,
            height: 600,
            title: title.to_string(),
            resizable: true,
            visible: true,
            blur: false,
            transparent: false,
            maximized: false,
            decorations: true,
        }
    }

    pub fn with_title(mut self, title: &str) -> Self {
        self.title = title.to_string();
        self
    }

    pub fn with_size(mut self, width: u32, height: u32) -> Self {
        self.width = width;
        self.height = height;
        self
    }

    pub fn with_resizable(mut self, resizable: bool) -> Self {
        self.resizable = resizable;
        self
    }

    pub fn with_visible(mut self, visible: bool) -> Self {
        self.visible = visible;
        self
    }

    pub fn with_blur(mut self, blur: bool) -> Self {
        self.blur = blur;
        self
    }

    pub fn with_transparent(mut self, transparent: bool) -> Self {
        self.transparent = transparent;
        self
    }

    pub fn with_maximized(mut self, maximized: bool) -> Self {
        self.maximized = maximized;
        self
    }

    pub fn with_decorations(mut self, decorations: bool) -> Self {
        self.decorations = decorations;
        self
    }
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            width: 800,
            height: 600,
            title: String::from("hydra"),
            resizable: false,
            visible: true,
            blur: false,
            transparent: false,
            maximized: false,
            decorations: true,
        }
    }
}

#[derive(Clone, Resource)]
pub struct Window {
    inner: Arc<winit::window::Window>,
    _unsend: std::marker::PhantomData<*const ()>,
}

impl Window {
    pub fn new(config: WindowConfig, event_loop: &ActiveEventLoop) -> Self {
        let attributes = winit::window::Window::default_attributes()
            .with_title(config.title)
            .with_inner_size(winit::dpi::PhysicalSize::new(config.width, config.height))
            .with_resizable(config.resizable)
            .with_visible(config.visible)
            .with_transparent(config.transparent)
            .with_maximized(config.maximized)
            .with_decorations(config.decorations);

        let window = event_loop.create_window(attributes).unwrap();

        Self {
            inner: Arc::new(window),
            _unsend: std::marker::PhantomData,
        }
    }

    pub fn id(&self) -> WindowId {
        self.inner.id()
    }

    pub fn size(&self) -> winit::dpi::PhysicalSize<u32> {
        self.inner.inner_size()
    }

    pub fn inner(&self) -> &winit::window::Window {
        &self.inner
    }
}
