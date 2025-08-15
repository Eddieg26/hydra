
struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) uv: vec2<f32>,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

struct Camera {
    view: mat4x4<f32>,
    projection: mat4x4<f32>,
}

struct Object {
    model: mat4x4<f32>,
}


@group(0) @binding(0) var<uniform> camera: Camera;
@group(1) @binding(0) var<storage, read> objects: array<Object>;

@vertex
fn vs_main(
    @location(0) position: vec2<f32>,
    @location(1) uv: vec2<f32>,
    @builtin(instance_index) instance_index: u32,
) -> VertexOutput {
    let model = objects[instance_index].model;

    var output: VertexOutput;
    output.position = camera.projection * camera.view * model * vec4<f32>(position, 0.0, 1.0);
    output.uv = uv;

    return output;
}


@group(2) @binding(0) var texture: texture_2d<f32>;
@group(2) @binding(1) var texture_sampler: sampler;

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    // Sample the texture using the UV coordinates
    let color = textureSample(texture, texture_sampler, input.uv);
    // Return the color from the texture
    return color;
}