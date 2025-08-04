

/// view projection matrix * view matrix
/// This matrix transforms world space coordinates into view space coordinates
@group(0) @binding(0) var<uniform> view_proj_matrix: mat4x4<f32>;

@vertex
fn vs_main(@location(0) position: vec3<f32>) -> @builtin(position) vec4<f32> {
    // Transform the vertex position into view space
    return view_proj_matrix * vec4<f32>(position, 1.0);
}