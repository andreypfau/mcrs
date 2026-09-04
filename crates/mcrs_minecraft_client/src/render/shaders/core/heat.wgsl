// Bound to the draw args, which the cull writes and the world pass reads, so the driver's
// hazard tracking runs the heat between one frame's world pass and the next frame's cull.
@group(0) @binding(0) var<storage, read_write> args: array<u32>;

const HEAT_THREADS: u32 = #{HEAT_THREADS}u;
const HEAT_STEPS: u32 = 4096u;
const FIRST_VERTEX: u32 = 2u;

@compute @workgroup_size(HEAT_THREADS)
fn heat(@builtin(global_invocation_id) id: vec3<u32>) {
    var x = f32(id.x) * 1e-3;
    for (var i = 0u; i < HEAT_STEPS; i = i + 1u) {
        x = fma(x, 0.999, 0.001);
    }
    args[FIRST_VERTEX] = select(0u, 1u, x == 123456.0);
}
