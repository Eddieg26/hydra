use ecs::App;
use render::plugins::RenderPlugin;

mod plugin;

fn main() {
    App::new().add_plugins(RenderPlugin).run();
}
