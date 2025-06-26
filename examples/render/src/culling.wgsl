
struct Frustum {
    planes: array<vec4<f32>, 6>,
}

struct Aabb {
    min: vec3<f32>,
    max: vec3<f32>
}

@group(1) @binding(0) var<storage, read> frustum: Frustum;

@group(1) @binding(1) var<storage, read> objects: array<Aabb>;

@group(1) @binding(2) var<storage, read_write> visible_objects: array<u32>;

@group(1) @binding(3) var<storage, read_write> current_offset: atomic<u32>;


@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {

    let object_index = id.x;

    if (object_index >= arrayLength(&objects)) {
        return;
    }
    
    let aabb = objects[object_index];

    var visible = true;
    for (var i = 0u; i < 6u; i = i + 1u) {
        let plane = frustum.planes[i];
        let positive = vec3<f32>(
            select(aabb.min.x, aabb.max.x, plane.x >= 0.0),
            
            select(aabb.min.y, aabb.max.y, plane.y >= 0.0),
            select(aabb.min.z, aabb.max.z, plane.z >= 0.0)
        );
        if (dot(plane.xyz, positive) + plane.w < 0.0) {
            visible = false;
            break;
        }
    }

    if (visible) {
        let output_index = atomicAdd(&current_offset, 1u);
        visible_objects[output_index] = object_index;
    }

}