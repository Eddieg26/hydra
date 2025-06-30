struct FragmentInput {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec3<f32>,
}

@group(2) @binding(0) var<uniform> value: vec4<f32>;

@fragment
fn main(input: FragmentInput) -> @location(0) vec4<f32> {
    return vec4<f32>(input.color, 1.0) + value;
}