// The coverage of each view image of an impostor image set: the share of the covered texels and
// their centroid, which the prepare pass uses for a small impostor.

@group(0) @binding(0) var colours: texture_2d_array<f32>;
@group(0) @binding(1) var<storage, read_write> covers: array<vec4<f32>>;
@group(0) @binding(2) var<uniform> cover_layer: vec4<u32>;

const TILE: u32 = #{TILE}u;
const YAWS: i32 = #{YAWS};
const VIEWS: u32 = #{VIEWS}u;

var<workgroup> covered: atomic<u32>;
var<workgroup> sum_x: atomic<u32>;
var<workgroup> sum_y: atomic<u32>;

/// One workgroup for each view.
@compute @workgroup_size(64)
fn cover(@builtin(workgroup_id) group: vec3<u32>, @builtin(local_invocation_index) local: u32) {
    let view = group.x;
    let layer = cover_layer.x;
    let origin = vec2<u32>(view % u32(YAWS), view / u32(YAWS)) * TILE;
    if (local == 0u) {
        atomicStore(&covered, 0u);
        atomicStore(&sum_x, 0u);
        atomicStore(&sum_y, 0u);
    }
    workgroupBarrier();
    var count = 0u;
    var x_sum = 0u;
    var y_sum = 0u;
    for (var texel = local; texel < TILE * TILE; texel = texel + 64u) {
        let at = vec2<u32>(texel % TILE, texel / TILE);
        if (textureLoad(colours, origin + at, layer, 0).a >= 0.5) {
            count = count + 1u;
            x_sum = x_sum + at.x;
            y_sum = y_sum + at.y;
        }
    }
    atomicAdd(&covered, count);
    atomicAdd(&sum_x, x_sum);
    atomicAdd(&sum_y, y_sum);
    workgroupBarrier();
    if (local == 0u) {
        let total = atomicLoad(&covered);
        var result = vec4<f32>(0.5, 0.5, 0.0, 0.0);
        if (total != 0u) {
            let mean = vec2<f32>(f32(atomicLoad(&sum_x)), f32(atomicLoad(&sum_y))) / f32(total);
            result = vec4<f32>((mean + 0.5) / f32(TILE), f32(total) / f32(TILE * TILE), 0.0);
        }
        covers[layer * VIEWS + view] = result;
    }
}
