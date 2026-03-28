// Full-screen triangle vertex + solid color fragment shader
// Used by ClearPass to fill the viewport with a camera's clear color

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
};

struct ClearColor {
    color: vec4<f32>,
};

@group(0) @binding(0)
var<uniform> clear: ClearColor;

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    // Full-screen triangle: 3 vertices cover [-1,1]x[-1,1] NDC
    let x = f32(i32(vertex_index & 1u) * 4 - 1);
    let y = f32(i32(vertex_index >> 1u) * 4 - 1);

    var out: VertexOutput;
    out.position = vec4<f32>(x, y, 0.0, 1.0);
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return clear.color;
}
