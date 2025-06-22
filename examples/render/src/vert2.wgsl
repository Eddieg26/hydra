struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) texcoord: vec2<f32>,
    @builtin(instance_index) instance_id: u32,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(1) texcoord: vec2<f32>,
}

struct Camera {
    world: mat4x4<f32>,
    view: mat4x4<f32>,
    projection: mat4x4<f32>,
}

struct Object {
    model: mat4x4<f32>,
};

@group(0) @binding(0) var<uniform> camera: Camera;
@group(1) @binding(0) var<storage, read> objects: array<Object>;

@vertex
fn main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;
    var object: mat4x4<f32> = objects[input.instance_id].model;

    output.position = camera.projection * camera.view * object * vec4<f32>(input.position, -1.0, 1.0);
    output.texcoord = input.texcoord;
    return output;
}