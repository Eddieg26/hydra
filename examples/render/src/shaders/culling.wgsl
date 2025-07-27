struct InstanceData {
    _padding_0: vec4<f32>,
    _padding_1: vec4<f32>,
    _padding_2: vec4<f32>,
}

struct Plane {
    normal: vec3<f32>,
    d: f32,
}

struct Aabb {
    min: vec3<f32>,
    max: vec3<f32>,
}

struct RenderEntity {
    world: mat4x4<f32>,  // Fix: was Mat4
    batch: u32,
    instance: u32,
}

struct View {
    frustum: array<Plane, 6>,
}

struct RenderBatch {
    bounds: Aabb,
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

@group(0) @binding(0) var<uniform> view: View;
@group(0) @binding(1) var<uniform> batch: RenderBatch;
@group(0) @binding(2) var<uniform> entity_count: u32;
@group(0) @binding(3) var<storage, read> instances: array<InstanceData>;
@group(0) @binding(4) var<storage, read> entities: array<RenderEntity>;
@group(0) @binding(5) var<storage, write> visible_count: atomic<u32>;
@group(0) @binding(6) var<storage, write> draw_args: array<DrawArgs>;
@group(0) @binding(7) var<storage, write> draw_indexed_args: array<DrawIndexedArgs>;
@group(0) @binding(8) var<storage, write> visible_instances: array<InstanceData>;

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let entity_index = global_id.x + batch.offset;
    if (entity_index >= entity_count) {
        return;
    }

    let entity = entities[entity_index];
    let bounds = batch.bounds;

    let transformed_bounds = transform_aabb(bounds.min, bounds.max, entity.world);
    let aabb_min = transformed_bounds[0];
    let aabb_max = transformed_bounds[1];
    let aabb_center = (aabb_min + aabb_max) * 0.5;
    let aabb_extents = (aabb_max - aabb_min) * 0.5;

    let planes = view.frustum;
    var visible = true;
    for (var i = 0; i < 6; i++) {
        let plane = planes[i];
        let distance = dot(plane.normal, aabb_center) + plane.d;
        if (distance < -dot(abs(plane.normal), aabb_extents)) {
            visible = false;
            break;
        }
    }

    if (visible) {
        let current_index = atomicAdd(&visible_count, 1);
        let instance = instances[entity.instance];
        visible_instances[current_index] = instance;

        if (batch.indexed == 0) {
            atomicAdd(&draw_args[entity.batch].instance_count, 1u);
            atomicMin(&draw_args[entity.batch].first_instance, current_index);
        } else {
            atomicAdd(&draw_indexed_args[entity.batch].instance_count, 1u);
            atomicMin(&draw_indexed_args[entity.batch].first_instance, current_index);
        }
    }
}

fn transform_aabb(aabb_min: vec3<f32>, aabb_max: vec3<f32>, transform: mat4x4<f32>) -> array<vec3<f32>, 2> {
    // Extract translation
    let translation = transform[3].xyz;
    
    // Start with translation
    var new_min = translation;
    var new_max = translation;
    
    // For each axis of the AABB
    for (var i = 0; i < 3; i++) {
        for (var j = 0; j < 3; j++) {
            let a = transform[j][i] * aabb_min[j];
            let b = transform[j][i] * aabb_max[j];
            new_min[i] += min(a, b);
            new_max[i] += max(a, b);
        }
    }
    
    return array<vec3<f32>, 2>(new_min, new_max);
}