use crate::plugin::{ DrawMesh, ExamplePlugin, LIT_COLOR_MAT, Light, PLANE, View3d};
use ecs::{App, AppTag, Component, Init, Spawner};
use math::Vec3;
use render::{Camera, Color, MeshFilter};
use transform::{GlobalTransform, Transform};

pub mod plugin;

#[derive(Component, Clone, AppTag)]
pub struct Test;

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
                    Transform::default().with_translation(Vec3::NEG_Z * 5.0 + Vec3::Y * 1.5),
                )
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
                .with_component(Transform::default().with_translation(Vec3::Y * 1.0))
                .with_component(GlobalTransform::default())
                .with_component(Light::default().with_color(Color::white()))
                .finish();
        })
        .run();
}
