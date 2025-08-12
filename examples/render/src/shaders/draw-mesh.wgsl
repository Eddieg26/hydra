struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) texcoord: vec2<f32>,
    @location(3) tangent: vec4<f32>,
    @builtin(instance_index) instance_id: u32,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) world_pos: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) texcoord: vec2<f32>,
    @location(3) tangent: vec4<f32>,
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
@group(1) @binding(1) var<storage, read> visible_list: array<u32>;

@vertex
fn main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;

    let index = visible_list[input.instance_id];
    let object: mat4x4<f32> = objects[index].model;
    
    // Calculate world position (model space -> world space)
    let world_position = object * vec4<f32>(input.position, 1.0);
    output.world_pos = world_position.xyz;
    
    // Calculate clip space position for rendering
    output.position = camera.projection * camera.view * world_position;
    
    // Transform normal and tangent to world space
    output.normal = (object * vec4<f32>(input.normal, 0.0)).xyz;
    output.tangent = (object * vec4<f32>(input.tangent.xyz, 0.0)).xyzw;
    output.tangent.w = input.tangent.w; // Preserve the handedness of the tangent

    output.texcoord = input.texcoord;

    return output;
}