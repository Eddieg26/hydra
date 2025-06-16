struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) texcoord: vec2<f32>,
    @location(3) tangent: vec4<f32>,
    @location(4) object_0: vec4<f32>,
    @location(5) object_1: vec4<f32>,
    @location(6) object_2: vec4<f32>,
    @location(7) object_3: vec4<f32>,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
}

struct Camera {
    world: mat4x4<f32>,
    view: mat4x4<f32>,
    projection: mat4x4<f32>,
}

@group(0) @binding(0) var<uniform> camera: Camera;

@vertex
fn main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;
    var object = mat4x4<f32>(
        input.object_0,
        input.object_1,
        input.object_2,
        input.object_3
    );
    output.position = camera.projection * camera.view * object * vec4<f32>(input.position, 1.0);
    return output;
}