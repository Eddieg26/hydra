use crate::{
    events::{
        AxisMotion, CursorEntered, CursorLeft, CursorMoved, DroppedFile, HoveredFile,
        HoveredFileCancelled, ModifiersChanged, ScaleFactorChanged, WindowClosed, WindowCreated,
        WindowDestroyed, WindowFocused, WindowMoved, WindowOccluded, WindowResized,
    },
    translate::{self},
    window::{Window, WindowConfig},
};
use ecs::{Apps, Command, Commands, Event, Events, Resource};
use input::{
    DoubleTapGesture, KeyboardInput, MouseInput, MouseScroll, PanGesture, PinchGesture,
    RotationGesture, TouchInput, TouchpadPressure,
};
use std::error::Error;
use winit::{
    application::ApplicationHandler,
    error::EventLoopError,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::WindowId,
};

pub struct WindowApp {
    apps: Apps,
}

impl WindowApp {
    pub fn new(apps: Apps) -> Self {
        Self { apps }
    }

    pub fn start(&mut self) {
        self.apps.init();
    }

    pub fn update(&mut self) -> Option<AppExit> {
        self.apps.run();
        self.apps.world_mut().remove_resource::<AppExit>()
    }

    pub fn shutdown(&mut self) {
        self.apps.shutdown();
    }

    fn send_event<E: Event>(&mut self, event: E) {
        self.apps
            .world_mut()
            .resource_mut::<Events<E>>()
            .writer()
            .send(event);
    }

    fn run(&mut self, event_loop: EventLoop<()>) {
        event_loop.set_control_flow(ControlFlow::Poll);

        if let Err(e) = event_loop.run_app(self) {
            let error = AppRunError::new(e);
            println!("{error}");
        }

        self.shutdown();
    }

    pub fn runner(apps: Apps) -> Apps {
        match EventLoop::new() {
            Ok(event_loop) => {
                let mut app = WindowApp::new(apps);
                app.run(event_loop);
                app.apps
            }
            Err(e) => {
                let error = AppRunError::new(e);
                println!("{error}");
                apps
            }
        }
    }
}

impl ApplicationHandler for WindowApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let has_window = self.apps.world().try_resource::<Window>().is_some();

        if !has_window {
            let world = self.apps.world_mut();
            let config = world.remove_resource::<WindowConfig>().unwrap_or_default();
            let window = Window::new(config, event_loop);
            let id = window.id();
            world.add_resource(window);
            world.send(WindowCreated::new(id));

            self.start();
        } else if let Some(exit) = self.update() {
            println!("Application exit: {}", exit);
            event_loop.exit();
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, window: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                self.send_event(WindowClosed::new(window));
                event_loop.exit();
            }
            WindowEvent::RedrawRequested => {
                if let Some(exit) = self.update() {
                    println!("Application exit: {}", exit);
                    event_loop.exit();
                } else {
                    let app = self.apps.world().resource::<Window>();
                    app.inner().request_redraw();
                }
            }
            WindowEvent::Destroyed => self.send_event(WindowDestroyed::new(window)),
            WindowEvent::Resized(size) => self.send_event(WindowResized::new(size)),
            WindowEvent::Moved(position) => self.send_event(WindowMoved::new(position)),
            WindowEvent::DroppedFile(path) => self.send_event(DroppedFile::new(path)),
            WindowEvent::HoveredFile(path) => self.send_event(HoveredFile::new(path)),
            WindowEvent::HoveredFileCancelled => self.send_event(HoveredFileCancelled),
            WindowEvent::Focused(focused) => self.send_event(WindowFocused::new(focused)),
            WindowEvent::KeyboardInput { event, .. } => {
                let event = KeyboardInput {
                    code: translate::physical_key(event.physical_key),
                    key: translate::logical_key(event.logical_key),
                    state: translate::button_state(event.state),
                    text: event.text,
                    repeat: event.repeat,
                };

                self.send_event(event);
            }
            WindowEvent::ModifiersChanged(modifiers) => {
                self.send_event(ModifiersChanged::new(modifiers))
            }
            WindowEvent::CursorMoved {
                device_id,
                position,
            } => self.send_event(CursorMoved::new(device_id, position)),
            WindowEvent::CursorEntered { device_id } => {
                self.send_event(CursorEntered::new(device_id))
            }
            WindowEvent::CursorLeft { device_id } => self.send_event(CursorLeft::new(device_id)),
            WindowEvent::AxisMotion {
                device_id,
                axis,
                value,
            } => self.send_event(AxisMotion::new(device_id, axis, value)),
            WindowEvent::MouseWheel { delta, .. } => {
                let delta = translate::mouse_delta(delta);
                self.send_event(MouseScroll { delta });
            }
            WindowEvent::MouseInput { state, button, .. } => {
                let input = MouseInput {
                    button: translate::mouse_button(button),
                    state: translate::button_state(state),
                };
                self.send_event(input);
            }
            WindowEvent::PinchGesture { delta, phase, .. } => {
                self.send_event(PinchGesture::new(delta, translate::touch_phase(phase)))
            }
            WindowEvent::PanGesture { delta, phase, .. } => self.send_event(PanGesture::new(
                delta.x,
                delta.y,
                translate::touch_phase(phase),
            )),
            WindowEvent::DoubleTapGesture { .. } => self.send_event(DoubleTapGesture),
            WindowEvent::RotationGesture { delta, phase, .. } => {
                self.send_event(RotationGesture::new(delta, translate::touch_phase(phase)))
            }
            WindowEvent::TouchpadPressure {
                pressure, stage, ..
            } => self.send_event(TouchpadPressure::new(pressure, stage)),
            WindowEvent::Touch(touch) => self.send_event(TouchInput::new(
                touch.id,
                translate::touch_phase(touch.phase),
                touch.location.x as f32,
                touch.location.y as f32,
                touch.force.map(translate::touch_force),
            )),
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                self.send_event(ScaleFactorChanged::new(scale_factor))
            }
            WindowEvent::Occluded(occluded) => self.send_event(WindowOccluded::new(occluded)),
            _ => {}
        }
    }
}

#[derive(Debug)]
pub struct AppRunError(EventLoopError);

impl AppRunError {
    fn new(error: EventLoopError) -> Self {
        Self(error)
    }

    pub fn error(&self) -> &EventLoopError {
        &self.0
    }
}

impl std::ops::Deref for AppRunError {
    type Target = EventLoopError;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::fmt::Display for AppRunError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.0, f)
    }
}

impl std::error::Error for AppRunError {}

#[derive(Debug, Resource)]
pub enum AppExit {
    Success,
    Error(Box<dyn Error + Send + Sync>),
}

impl AppExit {
    pub fn success() -> Self {
        AppExit::Success
    }

    pub fn error<E: Error + Send + Sync + 'static>(error: E) -> Self {
        AppExit::Error(Box::new(error))
    }

    pub fn is_success(&self) -> bool {
        matches!(self, AppExit::Success)
    }

    pub fn is_error(&self) -> bool {
        matches!(self, AppExit::Error(_))
    }
}

impl std::fmt::Display for AppExit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AppExit::Success => write!(f, "Application exited successfully"),
            AppExit::Error(e) => write!(f, "Application exited with error: {}", e),
        }
    }
}

impl Error for AppExit {}

impl Command for AppExit {
    fn execute(self, world: &mut ecs::World) {
        world.add_resource(self);
    }
}

pub trait WindowCommandsExt {
    fn exit(&mut self, exit: AppExit);

    fn exit_success(&mut self) {
        self.exit(AppExit::success());
    }
    fn exit_error<E: Error + Send + Sync + 'static>(&mut self, error: E) {
        self.exit(AppExit::error(error));
    }
}

impl WindowCommandsExt for Commands<'_, '_> {
    fn exit(&mut self, exit: AppExit) {
        self.add(exit);
    }
}
