struct FragmentInput {
    @builtin(position) position: vec4<f32>,
}

@group(2) @binding(0) var<uniform> color: vec4<f32>;

@fragment
fn main(input: FragmentInput) -> @location(0) vec4<f32> {
    return color;
}