// Full-screen triangle vertex + Reinhard tonemap fragment shader
// Used by OutputPassPlugin to convert HDR (Rgba16Float) → SDR (Bgra8Unorm / Bgra8UnormSrgb)

@group(0) @binding(0)
var input_texture: texture_2d<f32>;
@group(0) @binding(1)
var input_sampler: sampler;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    // Full-screen triangle: 3 vertices cover [-1,1]x[-1,1] NDC
    //   index 0 → (-1, -1)  uv (0, 1)
    //   index 1 → ( 3, -1)  uv (2, 1)
    //   index 2 → (-1,  3)  uv (0, -1)
    let x = f32(i32(vertex_index & 1u) * 4 - 1);
    let y = f32(i32(vertex_index >> 1u) * 4 - 1);

    var out: VertexOutput;
    out.position = vec4<f32>(x, y, 0.0, 1.0);
    out.uv = vec2<f32>((x + 1.0) * 0.5, (1.0 - y) * 0.5);
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let hdr_color = textureSample(input_texture, input_sampler, in.uv);

    // Reinhard tonemapping: L_out = L_in / (1 + L_in)
    let mapped = hdr_color.rgb / (vec3<f32>(1.0) + hdr_color.rgb);

    return vec4<f32>(mapped, hdr_color.a);
}
