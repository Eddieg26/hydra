use ecs::App;
use render::plugin::RenderPlugin;

fn main() {
    App::new().add_plugins(RenderPlugin).run();
}
