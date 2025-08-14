struct Plane {
    normal: vec3<f32>,
    d: f32,
}

struct Aabb {
    min: vec3<f32>,
    max: vec3<f32>,
}

struct Frustum {
    planes: array<Plane, 6>,
}

struct Object {
    world: mat4x4<f32>,
};

struct BatchInfo {
    bounds: Aabb,
    index: u32,
    offset: u32,
    count: u32,
    indexed: u32,
}

struct DrawArgs {
    vertex_count: u32,
    instance_count: atomic<u32>,
    first_vertex: u32,
    first_instance: atomic<u32>,
}

struct DrawIndexedArgs {
    index_count: u32,
    instance_count: atomic<u32>,
    first_index: u32,
    base_vertex: i32,
    first_instance: atomic<u32>,
}

@group(0) @binding(0) var<uniform> frustum: Frustum;

@group(1) @binding(0) var<uniform> batch: BatchInfo;

@group(2) @binding(0) var<storage, read> objects: array<Object>;

@group(2) @binding(1) var<storage, read_write> visible_list: array<u32>;

@group(2) @binding(2) var<storage, read_write> visible_count: atomic<u32>;

@group(3) @binding(0) var<storage, read_write> draw_args: array<DrawArgs>;

@group(3) @binding(1) var<storage, read_write> draw_indexed_args: array<DrawIndexedArgs>;


@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let object_index = global_id.x + batch.offset;

    if object_index >= arrayLength(&objects) {
        return;
    }

    let object = objects[object_index];
    let bounds = transform_bounds(object.world, batch.bounds);
    let aabb_center = (bounds.min + bounds.max) * 0.5;
    let aabb_extents = (bounds.max - bounds.min) * 0.5;

    // Check if the AABB intersects with the frustum
    var inside = true;
    for (var i = 0u; i < 6u; i++) {
        let plane = frustum.planes[i];
        let dist = dot(plane.normal, aabb_center) + plane.d;
        if dist < -dot(abs(plane.normal), aabb_extents) {
            inside = false;
            break;
        }
    }

    if inside {
        let count = atomicAdd(&visible_count, 1u);
        visible_list[count] = object_index;

        if batch.indexed == 0u {
            atomicAdd(&draw_args[batch.index].instance_count, 1u);
            atomicMin(&draw_args[batch.index].first_instance, count);
        } else {
            atomicAdd(&draw_indexed_args[batch.index].instance_count, 1u);
            atomicMin(&draw_indexed_args[batch.index].first_instance, count);
        }
    }
}

fn transform_bounds(world: mat4x4<f32>, bounds: Aabb) -> Aabb {
    var corners = array<vec3<f32>, 8>(
        vec3<f32>(bounds.min.x, bounds.min.y, bounds.min.z),
        vec3<f32>(bounds.max.x, bounds.min.y, bounds.min.z),
        vec3<f32>(bounds.min.x, bounds.max.y, bounds.min.z),
        vec3<f32>(bounds.max.x, bounds.max.y, bounds.min.z),
        vec3<f32>(bounds.min.x, bounds.min.y, bounds.max.z),
        vec3<f32>(bounds.max.x, bounds.min.y, bounds.max.z),
        vec3<f32>(bounds.min.x, bounds.max.y, bounds.max.z),
        vec3<f32>(bounds.max.x, bounds.max.y, bounds.max.z),
    );

    // Transform first corner to initialize bounds
    let first_transformed = (world * vec4<f32>(corners[0], 1.0)).xyz;
    var transformed_bounds: Aabb;
    transformed_bounds.min = first_transformed;
    transformed_bounds.max = first_transformed;

    // Transform remaining corners and expand bounds
    for (var i = 1u; i < 8u; i++) {
        let transformed_corner = (world * vec4<f32>(corners[i], 1.0)).xyz;
        transformed_bounds.min = min(transformed_bounds.min, transformed_corner);
        transformed_bounds.max = max(transformed_bounds.max, transformed_corner);
    }

    return transformed_bounds;
}