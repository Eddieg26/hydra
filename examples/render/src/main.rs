use ecs::{App, Init, NonSend};
use render::plugin::RenderPlugin;
use window::{Window, plugin::WindowPlugin};

fn main() {
    App::new().add_plugins(RenderPlugin).run();
}
