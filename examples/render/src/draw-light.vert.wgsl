struct VertexInput {
    @location(0) position: vec3<f32>,
    @builtin(instance_index) instance_id: u32,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec3<f32>,
}

struct Camera {
    world: mat4x4<f32>,
    view: mat4x4<f32>,
    projection: mat4x4<f32>,
}

struct Light {
    position: vec3<f32>,
    range: f32,
    color: vec3<f32>,
    intensity: f32,
};

@group(0) @binding(0) var<uniform> camera: Camera;
@group(1) @binding(0) var<storage, read> lights: array<Light>;

@vertex
fn main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;

    let light = lights[input.instance_id];
    let position = (light.position + light.range / 2.0 * input.position);
    output.position = camera.projection * camera.view * vec4<f32>(position, 1.0);
    output.color = light.color;
    return output;
}