use crate::plugin::{DrawMesh, ExamplePlugin, Light, View3d, CUBE, LIT_COLOR_MAT, PLANE};
use ecs::{App, Init, Spawner};
use math::Vec3;
use render::{Camera, Color, MeshFilter};
use transform::{GlobalTransform, Transform};

pub mod plugin;

fn main() {
    App::new()
        .add_plugins(ExamplePlugin)
        .add_systems(Init, |mut spawner: Spawner| {
            spawner
                .spawn()
                .with_component(Camera {
                    clear_color: Some(Color::blue()),
                    ..Default::default()
                })
                .with_component(Transform::default().with_translation(Vec3::NEG_Z * 5.0 + Vec3::Y * 1.5))
                .with_component(GlobalTransform::default())
                .with_component(View3d::default())
                .finish();

            spawner
                .spawn()
                .with_component(Transform::default().with_rotation(Vec3::X * -90.0))
                .with_component(GlobalTransform::default())
                .with_component(MeshFilter::from(PLANE))
                .with_component(DrawMesh::from(LIT_COLOR_MAT))
                .finish();

            spawner
                .spawn()
                .with_component(Transform::default().with_translation(Vec3::Y * 9.0))
                .with_component(GlobalTransform::default())
                .with_component(Light::default().with_color(Color::red()))
                .finish();
        })
        .run();
}
