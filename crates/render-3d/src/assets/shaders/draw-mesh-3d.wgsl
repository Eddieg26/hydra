//#define MAX_INSTANCES 100

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) tangent: vec4<f32>,
    @builtin(instance_index) instance_index: u32,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) tangent: vec4<f32>,
}


struct CameraUniform {
    view_projection: mat4x4<f32>,
    view: mat4x4<f32>,
    projection: mat4x4<f32>,
    world_position: vec4<f32>,
}

struct Instance {
    world: mat4x4<f32>,
}


@group(0) @binding(0)
var<uniform> camera: CameraUniform;

#ifdef STORAGE_ENABLED
@group(1) @binding(0)
var<storage, read> instances: array<Instance>;
#else
var<uniform> instances: array<Instance, 100>;

@vertex
fn vertex_main(
    input: VertexInput
) -> VertexOutput {
    let instance = instances[input.instance_index];
    let world_position = camera.view * instance.world * vec4<f32>(input.position, 1.0);
    let clip_position = camera.view_projection * world_position;
    let normal = normalize((camera.view * instance.world * vec4<f32>(input.normal, 0.0)).xyz);
    let tangent = normalize((camera.view * instance.world * vec4<f32>(input.tangent.xyz, 0.0)).xyzw);
    tangent.w = input.tangent.w; // Preserve the handedness of the tangent

    return VertexOutput(
        clip_position,
        world_position.xyz,
        normal,
        input.uv,
        tangent
    );
}