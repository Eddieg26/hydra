use crate::plugin::ExamplePlugin;
use ecs::App;

mod plugin;

fn main() {
    App::new().add_plugins(ExamplePlugin).run();
}
