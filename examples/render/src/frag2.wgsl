struct FragmentInput {
    @builtin(position) position: vec4<f32>,
    @location(1) tex_coord: vec2<f32>,
}

@group(2) @binding(0) var texture: texture_2d<f32>;
@group(2) @binding(1) var tex_sampler: sampler;

@fragment
fn main(input: FragmentInput) -> @location(0) vec4<f32> {
    let color: vec4<f32> = textureSample(texture, tex_sampler, input.tex_coord);
    return color;
}