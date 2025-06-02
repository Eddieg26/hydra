use ecs::App;
use window::plugin::WindowPlugin;

fn main() {
    App::new().add_plugins(WindowPlugin).run();
}
