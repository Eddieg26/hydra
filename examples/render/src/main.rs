// const VERT_ID: AssetId<Shader> = AssetId::from_u128(0xabcdef0123456789);
// const VERT_ID_2: AssetId<Shader> = AssetId::from_u128(0x7fa18a3696e84df5848822a3b417e3f3u128);
// const DRAW_LIGHT_VERT: AssetId<Shader> = AssetId::from_u128(0x9e08450b1c394c8c88de79b6aa2c2589);
// const DRAW_LIGHT_FRAG: AssetId<Shader> = AssetId::from_u128(0x7503fc9b9eda4559a7fde1459f02058b);
// const FRAG_ID: AssetId<Shader> = AssetId::from_u128(0x123456789abcdef0);
// const FRAG_ID_2: AssetId<Shader> = AssetId::from_u128(0x9876543210fedcba);
// const FRAG_ID_3: AssetId<Shader> = AssetId::from_u128(0x3e7c2a1b4f5e4c2e9d1a8b7e6c5d4f3a);
// const UNLIT_WHITE: AssetId<UnlitColor> = AssetId::from_u128(0xfca61c1a76b14268b25058d36dbc6389);
// const UNLIT_RED: AssetId<UnlitColor> = AssetId::from_u128(0xa0cc79971c2d4206874539cb5ac54fe2u128);
// const UNLIT_BLUE: AssetId<UnlitColor> = AssetId::from_u128(0x87654321fedcba98);
// const LIT_WHITE: AssetId<LitColor> = AssetId::from_u128(0x9a8b7c6d5e4f3a2b1c0d8e7f6a5b4c3d);
// const LIT_RED: AssetId<LitColor> = AssetId::from_u128(0x1a2b3c4d5e6f708192a0b1c2d3e4f506u128);
// const LIGHT_MAT: AssetId<LightMaterial> = AssetId::from_u128(0xcd9c7e475e84435db8316d2612b94e2d);
// const QUAD_ID: AssetId<Mesh> = AssetId::from_u128(0xe51f72d138f747c6b22e2ac8a64b7b92u128);
// const CUBE_ID: AssetId<Mesh> = AssetId::from_u128(0x9d3919f428f8429a80e195849b3b6c21u128);
// const PLANE_ID: AssetId<Mesh> = AssetId::from_u128(0x2b3c4d5e6f708192a0b1c2d3e4f50607u128);
// const SWORD_ID: AssetId<Mesh> = AssetId::from_u128(0x6d3d79f5c6764b43993ae8de7ed0219bu128);
// const GENGAR_ID: AssetId<Texture> = AssetId::from_u128(0x43c5893d2b2f4a3bb2bb33eb1b362ff6u128);
// // const UNLIT_TEX_ID: AssetId<UnlitTexture> =
// //     AssetId::from_u128(0x1a2b3c4d5e6f708192a0b1c2d3e4f506u128);

// const QUAD: &[math::Vec2] = &[
//     math::Vec2::new(-0.5, -0.5), // Bottom-left
//     math::Vec2::new(0.5, -0.5),  // Bottom-right
//     math::Vec2::new(-0.5, 0.5),  // Top-left
//     math::Vec2::new(0.5, -0.5),  // Bottom-right
//     math::Vec2::new(0.5, 0.5),   // Top-right
//     math::Vec2::new(-0.5, 0.5),  // Top-left
// ];

// const QUAD_TEX_COORDS: &[math::Vec2] = &[
//     math::Vec2::new(0.0, 1.0), // Bottom-left
//     math::Vec2::new(1.0, 1.0), // Bottom-right
//     math::Vec2::new(0.0, 0.0), // Top-left
//     math::Vec2::new(1.0, 1.0), // Bottom-right
//     math::Vec2::new(1.0, 0.0), // Top-right
//     math::Vec2::new(0.0, 0.0), // Top-left
// ];

use asset::{AssetId, embed_asset, io::EmbeddedFs, plugin::AssetAppExt};
use ecs::{App};
use render::{Mesh, MeshSettings, Shader, ShaderSettings};

const COMMON_SHADER: AssetId<Shader> = AssetId::from_u128(0x3e7c2a1b4f5e4c2e9d1a8b7e6c5d4f3a);
const DRAW_MESH_SHADER: AssetId<Shader> = AssetId::from_u128(0xabcdef0123456789);
const UNLIT_COLOR_SHADER: AssetId<Shader> =
    AssetId::from_u128(0x7fa18a3696e84df5848822a3b417e3f3u128);
const UNLIT_TEX_SHADER: AssetId<Shader> = AssetId::from_u128(0x9e08450b1c394c8c88de79b6aa2c2589);
const CUBE: AssetId<Mesh> = AssetId::from_u128(0x123456789abcdef0);
const PLANE: AssetId<Mesh> = AssetId::from_u128(0xfca61c1a76b14268b25058d36dbc6389);

fn main() {
    let embedded = EmbeddedFs::new();
    embed_asset!(
        embedded,
        COMMON_SHADER,
        "shaders/common.wgsl",
        ShaderSettings::default()
    );

    embed_asset!(
        embedded,
        DRAW_MESH_SHADER,
        "shaders/draw-mesh.wgsl",
        ShaderSettings::default()
    );

    embed_asset!(
        embedded,
        UNLIT_COLOR_SHADER,
        "shaders/unlit-color.wgsl",
        ShaderSettings::default()
    );

    embed_asset!(
        embedded,
        UNLIT_TEX_SHADER,
        "shaders/unlit-texture.wgsl",
        ShaderSettings::default()
    );

    embed_asset!(embedded, CUBE, "meshes/cube.obj", MeshSettings::default());
    embed_asset!(embedded, PLANE, "meshes/plane.obj", MeshSettings::default());

    App::new()
        .add_source("embedded", embedded)
        .load_asset::<Mesh>(CUBE)
        .load_asset::<Mesh>(PLANE)
        .run();
}
