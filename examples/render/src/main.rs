use crate::plugin::{CUBE, DrawMesh, ExamplePlugin, UNLIT_COLOR_MAT, View3d};
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
                .with_component(Transform::default().with_translation(Vec3::NEG_Z * 5.0))
                .with_component(GlobalTransform::default())
                .with_component(View3d::default())
                .finish();

            spawner
                .spawn()
                .with_component(Transform::default())
                .with_component(GlobalTransform::default())
                .with_component(MeshFilter::from(CUBE))
                .with_component(DrawMesh::from(UNLIT_COLOR_MAT))
                .finish();
        })
        .run();
}
