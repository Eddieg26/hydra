#import embedded://shaders/common.wgsl

struct FragmentInput {
    @location(0) world_pos: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) texcoord: vec2<f32>,
}

struct PointLight {
    position: vec3<f32>,
    range: f32,
    color: vec3<f32>,
    intensity: f32,
    // view_space_matrix: mat4x4<f32>
};

@group(2) @binding(0) var<uniform> albedo: vec4<f32>;

@group(3) @binding(0)
var<uniform> point_lights: array<PointLight, BATCH_SIZE>;

@group(3) @binding(1)
var<uniform> light_count: u32;

// @group(3) @binding(2)
// var shadow_map: texture_depth_2d;

// @group(3) @binding(3)
// var shadow_sampler: sampler;

@fragment
fn main(input: FragmentInput) -> @location(0) vec4<f32> {
    let color: vec3<f32> = compute_diffuse_lighting(input.world_pos, normalize(input.normal), albedo.rgb);
    return vec4<f32>(color, 1.0);
}

fn compute_diffuse_lighting(
    world_pos: vec3<f32>,
    normal: vec3<f32>,
    albedo: vec3<f32>,
) -> vec3<f32> {
    var color = vec3<f32>(0.0);

    for (var i = 0u; i < light_count; i++) {
        let light = point_lights[i];
        let light_dir = light.position - world_pos;
        let distance = length(light_dir);
        let L = normalize(light_dir);

        let NdotL = max(dot(normal, L), 0.0);

        // Range-based attenuation
        let attenuation = clamp(1.0 - distance / light.range, 0.0, 1.0);
        let att = attenuation * attenuation;

        // let shadow = calculate_shadow(light.view_space_matrix * vec4<f32>(world_pos, 1.0));

        color += albedo * light.color * NdotL * att * light.intensity;
    }

    return color;
}

// fn calculate_shadow(frag_position: vec4<f32>) -> f32 {
//      // Transform the fragment position into shadow map space
//     let shadow_coord = frag_position.xyz / frag_position.w;

//     // NDC [-1,1] -> [0,1]
//     let proj_coord = shadow_coord * 0.5 + 0.5;

//     // Sample the shadow map
//     let closest_depth = textureSample(shadow_map, shadow_sampler, proj_coord.xy).r;

//     // Compare the depth value to determine if in shadow
//     return if closest_depth < proj_coord.z { 0.0 } else { 1.0 };
// }
