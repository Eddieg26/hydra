use ecs::{App, Init, Spawner};
use math::Vec3;
use render::{Camera, Color, MeshFilter, plugins::RenderPlugin};
use transform::{GlobalTransform, Transform};

use crate::plugin::{
    CUBE, DrawMesh, ExamplePlugin, LIT_COLOR_RED, LIT_COLOR_WHITE, Light, PLANE, SPHERE, SWORD,
    UNLIT_COLOR_MAT, View3d,
};

mod plugin;

fn main() {
    App::new()
        .add_plugins(ExamplePlugin)
        .add_systems(Init, |mut spawner: Spawner| {
            spawner
                .spawn()
                .with_component(Camera {
                    clear_color: Some(Color::black()),
                    ..Default::default()
                })
                .with_component(
                    Transform::default().with_translation(Vec3::NEG_Z * 10.0 + Vec3::Y * 4.0),
                )
                .with_component(GlobalTransform::default())
                .with_component(View3d::default())
                .finish();

            spawner
                .spawn()
                .with_component(
                    Transform::default()
                        .with_rotation(Vec3::X * -90.0f32.to_radians())
                        .with_scale(Vec3::new(5.0, 1.0, 5.0)),
                )
                .with_component(GlobalTransform::default())
                .with_component(MeshFilter::from(PLANE))
                .with_component(DrawMesh::from(LIT_COLOR_WHITE))
                .finish();

            spawner
                .spawn()
                .with_component(Transform::default().with_translation(Vec3::Y * 1.5))
                .with_component(GlobalTransform::default())
                .with_component(MeshFilter::from(SPHERE))
                .with_component(DrawMesh::from(LIT_COLOR_RED))
                .finish();

            spawner
                .spawn()
                .with_component(Transform::default().with_translation(Vec3::Y * 5.0))
                .with_component(GlobalTransform::default())
                .with_component(Light::default().with_color(Color::white()))
                .finish();
        })
        .run();
}
