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
};

@group(2) @binding(0) var<uniform> albedo: vec4<f32>;

@group(3) @binding(0)
var<storage, read> point_lights: array<PointLight>;

@group(3) @binding(1)
var<uniform> light_count: u32;

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

        //Range-based attenuation
        let attenuation = clamp(1.0 - distance / light.range, 0.0, 1.0);
        let att = attenuation * attenuation;

        color += albedo * light.color * NdotL * att * light.intensity;
    }

    return color;
}
