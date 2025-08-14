use ecs::App;
use crate::plugin::ExamplePlugin;

mod plugin;

fn main() {
    App::new().add_plugins(ExamplePlugin).run();
}
