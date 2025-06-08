use asset::Asset;
use ecs::App;
use render::{AsBinding, Color, ShaderType, plugin::RenderPlugin};

fn main() {
    App::new().add_plugins(RenderPlugin).run();
}

#[derive(Clone, Asset, AsBinding)]
#[uniform(0)]
pub struct UnlitColor {
    #[uniform]
    color: Color,
}

#[derive(ShaderType)]
struct Test {
    value: u32,
}
