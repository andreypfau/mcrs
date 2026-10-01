
#import mcrs_minecraft_client::fields::{
    SECTION_SIZE,
    BOUNDS_HI_X_SHIFT, BOUNDS_HI_X_BITS, BOUNDS_HI_Y_SHIFT, BOUNDS_HI_Y_BITS,
    BOUNDS_HI_Z_SHIFT, BOUNDS_HI_Z_BITS, BOUNDS_LO_X_SHIFT, BOUNDS_LO_X_BITS,
    BOUNDS_LO_Y_SHIFT, BOUNDS_LO_Y_BITS, BOUNDS_LO_Z_SHIFT, BOUNDS_LO_Z_BITS,
    FACE_NONE, MODEL_OVERHANG, MODEL_X_WORD, MODEL_Y_WORD, MODEL_Z_WORD, QUAD_WORDS,
}
#import mcrs_minecraft_client::frame::{PARAMS_MODEL, PARAMS_QUAD_CULL, camera, params}
#import mcrs_minecraft_client::quad::{
    CORNERS_PER_QUAD, MODEL_WORDS_PER_VERTEX, face_normal, greedy_corner, model_position,
}
#import mcrs_minecraft_client::section::{SectionDesc, section_origin}

struct Group {
    quad_base: u32,
    quad_count: u32,
    section: u32,
    face: u32,
    bounds: u32,
}

struct DrawArgs {
    vertex_count: atomic<u32>,
    instance_count: u32,
    first_vertex: u32,
    first_instance: u32,
}

const VERTICES_PER_QUAD: u32 = 6u;

@group(1) @binding(0) var<storage, read> groups: array<Group>;
@group(1) @binding(1) var<storage, read_write> visible: array<vec2<u32>>;
@group(1) @binding(2) var<storage, read_write> args: array<DrawArgs>;
/// Two bitsets over the section table: the cave walk's, then this frame's, which also leaves out
/// every section off the screen.
@group(1) @binding(3) var<storage, read_write> cave_visible: array<u32>;
@group(1) @binding(4) var<storage, read> sections: array<SectionDesc>;
@group(1) @binding(5) var<storage, read_write> batches: array<u32>;
/// The depth pyramid: last frame's for the first pass, rebuilt from what the first pass drew
/// before the second.
@group(1) @binding(6) var hiz: texture_2d<f32>;
/// Two words a group: which of its quads the first pass left to the second, a bit a quad, so a
/// group holds no more quads than a word has bits; and a place in the list of groups a pass's
/// group cull kept.
@group(1) @binding(7) var<storage, read_write> candidates: array<u32>;
@group(1) @binding(8) var<storage, read> quad_words: array<u32>;
@group(1) @binding(9) var<storage, read> model_words: array<u32>;

/// The dispatch does not scale with the world: a fixed grid of workgroups strides over the group
/// arena a batch of `CULL_THREADS` groups at a time, so a resident world of any size is covered
/// by the same command and the 65535-per-dimension cap on a dispatch can never be reached.
const CULL_THREADS: u32 = #{CULL_THREADS}u;
const GROUP_THREADS: u32 = #{GROUP_THREADS}u;
const STREAMS: u32 = #{STREAMS}u;
const CAVE_WORDS: u32 = #{CAVE_WORDS}u;
const SECTION_SLOTS: u32 = #{SECTION_SLOTS}u;
/// The most workgroups one dimension of a dispatch may ask for. A kernel reading a list strides
/// over it, so a list longer than this still gets read whole.
const MAX_WORKGROUPS: u32 = #{MAX_WORKGROUPS}u;
const NO_SLOT: u32 = 0xffffffffu;


var<workgroup> batch: array<Group, CULL_THREADS>;
var<workgroup> counts: array<u32, CULL_THREADS>;
var<workgroup> starts: array<u32, CULL_THREADS>;
var<workgroup> reserved: u32;
var<workgroup> shown_word: atomic<u32>;

fn in_frustum(mn: vec3<f32>, mx: vec3<f32>) -> bool {
    for (var i = 0u; i < 5u; i = i + 1u) {
        let plane = camera.frustum[i];
        let corner = vec3<f32>(
            select(mn.x, mx.x, plane.x > 0.0),
            select(mn.y, mx.y, plane.y > 0.0),
            select(mn.z, mx.z, plane.z > 0.0),
        );
        if (dot(plane.xyz, corner) + plane.w <= 0.0) {
            return false;
        }
    }
    return true;
}

fn faces_camera(face: u32, mn: vec3<f32>, mx: vec3<f32>) -> bool {
    if (face >= FACE_NONE) {
        return true;
    }
    let n = face_normal(face);
    let nearest = select(mx, mn, n > vec3<f32>(0.0));
    return dot(n, camera.offset - nearest) > 0.0;
}

struct Box {
    mn: vec3<f32>,
    mx: vec3<f32>,
}

fn group_box(g: Group) -> Box {
    let desc = sections[g.section];
    let origin = section_origin(desc);
    let scale = f32(desc.scale);
    let lo = vec3<f32>(
        f32(extractBits(g.bounds, BOUNDS_LO_X_SHIFT, BOUNDS_LO_X_BITS)),
        f32(extractBits(g.bounds, BOUNDS_LO_Y_SHIFT, BOUNDS_LO_Y_BITS)),
        f32(extractBits(g.bounds, BOUNDS_LO_Z_SHIFT, BOUNDS_LO_Z_BITS)),
    ) - MODEL_OVERHANG;
    let hi = vec3<f32>(
        f32(extractBits(g.bounds, BOUNDS_HI_X_SHIFT, BOUNDS_HI_X_BITS)),
        f32(extractBits(g.bounds, BOUNDS_HI_Y_SHIFT, BOUNDS_HI_Y_BITS)),
        f32(extractBits(g.bounds, BOUNDS_HI_Z_SHIFT, BOUNDS_HI_Z_BITS)),
    ) - MODEL_OVERHANG;
    return Box(origin + lo * scale, origin + hi * scale);
}

fn section_shown(slot: u32) -> bool {
    return ((cave_visible[CAVE_WORDS + (slot >> 5u)] >> (slot & 31u)) & 1u) != 0u;
}

fn survives(g: Group, bounds: Box) -> bool {
    return in_frustum(bounds.mn, bounds.mx) && faces_camera(g.face, bounds.mn, bounds.mx);
}

/// A section's whole reach, models leaning out of it included.
fn section_box(slot: u32) -> Box {
    let desc = sections[slot];
    let origin = section_origin(desc);
    let span = SECTION_SIZE * f32(desc.scale);
    return Box(origin - MODEL_OVERHANG, origin + span + MODEL_OVERHANG);
}

/// Once a frame, before any group is read: a bit a section the cave walk reaches and the frustum
/// holds. A group of a section left out is dropped on that one bit, without reading anything of
/// the section's own.
@compute @workgroup_size(CULL_THREADS)
fn cull_sections(
    @builtin(workgroup_id) workgroup: vec3<u32>,
    @builtin(num_workgroups) grid: vec3<u32>,
    @builtin(local_invocation_index) local: u32,
) {
    var first = workgroup.x * CULL_THREADS;
    loop {
        if (first >= SECTION_SLOTS) {
            break;
        }
        let slot = first + local;
        if (local == 0u) {
            atomicStore(&shown_word, 0u);
        }
        workgroupBarrier();
        if (slot < SECTION_SLOTS && ((cave_visible[slot >> 5u] >> (slot & 31u)) & 1u) != 0u) {
            let bounds = section_box(slot);
            if (in_frustum(bounds.mn, bounds.mx)) {
                atomicOr(&shown_word, 1u << (slot & 31u));
            }
        }
        workgroupBarrier();
        if (local == 0u) {
            cave_visible[CAVE_WORDS + (first >> 5u)] = atomicLoad(&shown_word);
        }
        first = first + grid.x * CULL_THREADS;
    }
}

/// Where a shape lands on the screen: its box in normalised device coordinates and its nearest
/// depth, or `in_front` false when a corner lies behind the camera and no box can be drawn.
struct Footprint {
    lo: vec2<f32>,
    hi: vec2<f32>,
    nearest: f32,
    in_front: bool,
}

fn empty_footprint() -> Footprint {
    return Footprint(vec2<f32>(1e30), vec2<f32>(-1e30), 0.0, true);
}

const MODEL_BIAS_CONSTANT: f32 = #{MODEL_BIAS_CONSTANT};
/// A shader def holds no float, so the slope scale comes as its bits.
const MODEL_BIAS_SLOPE_BITS: u32 = #{MODEL_BIAS_SLOPE_BITS}u;
/// The spacing of f32 depths just below one, as a share of the depth: the unit of a depth bias
/// and of the rounding between where the cull projects a corner and where the rasterizer puts it.
const DEPTH_STEP: f32 = 1.0 / 8388608.0;

/// How far a triangle's depth changes across a pixel at most, which scales a depth bias.
fn depth_slope(a: vec3<f32>, b: vec3<f32>, c: vec3<f32>) -> f32 {
    let half = 0.5 * camera.viewport;
    let e1 = (b.xy - a.xy) * half;
    let e2 = (c.xy - a.xy) * half;
    let det = e1.x * e2.y - e1.y * e2.x;
    if (abs(det) < 1e-6) {
        return 0.0;
    }
    let dz1 = b.z - a.z;
    let dz2 = c.z - a.z;
    return max(abs(dz1 * e2.y - dz2 * e1.y), abs(e1.x * dz2 - e2.x * dz1)) / abs(det);
}

fn cover(footprint: ptr<function, Footprint>, corner: vec3<f32>) {
    let clip = camera.clip_from_relative * vec4<f32>(corner, 1.0);
    if (clip.w <= 0.0) {
        (*footprint).in_front = false;
        return;
    }
    let ndc = clip.xyz / clip.w;
    (*footprint).lo = min((*footprint).lo, ndc.xy);
    (*footprint).hi = max((*footprint).hi, ndc.xy);
    (*footprint).nearest = max((*footprint).nearest, ndc.z);
}

/// The pyramid halves each level and hands an odd leftover row or column to its last texel, so a
/// pixel lies under texel `min(pixel >> (level + 1), size - 1)` of a level. Scaling a coordinate by
/// a level's size instead drifts off that by up to a texel, and reads a texel beside the box.
fn farthest_under(pyramid: texture_2d<f32>, lo: vec2<f32>, hi: vec2<f32>) -> f32 {
    let uv_lo = vec2<f32>(lo.x, -hi.y) * 0.5 + 0.5;
    let uv_hi = vec2<f32>(hi.x, -lo.y) * 0.5 + 0.5;
    let pixels = vec2<u32>(camera.viewport);
    let base = textureDimensions(pyramid, 0);
    let b_lo = min(min(vec2<u32>(uv_lo * camera.viewport), pixels - 1u) >> vec2<u32>(1u), base - 1u);
    let b_hi = min(min(vec2<u32>(uv_hi * camera.viewport), pixels - 1u) >> vec2<u32>(1u), base - 1u);
    let extent = b_hi - b_lo + 1u;
    let level = min(u32(ceil(log2(f32(max(extent.x, extent.y))))), camera.hiz_levels - 1u);
    let last = textureDimensions(pyramid, level) - vec2<u32>(1u);
    let t_lo = min(b_lo >> vec2<u32>(level), last);
    let t_hi = min(b_hi >> vec2<u32>(level), last);
    var farthest = 1.0;
    for (var y = t_lo.y; y <= t_hi.y; y = y + 1u) {
        for (var x = t_lo.x; x <= t_hi.x; x = x + 1u) {
            farthest = min(farthest, textureLoad(pyramid, vec2<u32>(x, y), level).r);
        }
    }
    return farthest;
}

/// Whether the pyramid holds something nearer than every point of the footprint over the whole of
/// it. A shape reaching behind the camera or past the edge of the screen is never hidden: what
/// the pyramid does not cover says nothing about it.
fn hidden_in(pyramid: texture_2d<f32>, footprint: Footprint) -> bool {
    if (camera.hiz_levels == 0u || !footprint.in_front) {
        return false;
    }
    if (any(footprint.lo < vec2<f32>(-1.0)) || any(footprint.hi > vec2<f32>(1.0))) {
        return false;
    }
    return footprint.nearest < farthest_under(pyramid, footprint.lo, footprint.hi);
}

fn box_footprint(mn: vec3<f32>, mx: vec3<f32>) -> Footprint {
    var footprint = empty_footprint();
    for (var i = 0u; i < 8u; i = i + 1u) {
        cover(&footprint, vec3<f32>(
            select(mn.x, mx.x, (i & 1u) != 0u),
            select(mn.y, mx.y, (i & 2u) != 0u),
            select(mn.z, mx.z, (i & 4u) != 0u),
        ));
    }
    return footprint;
}


/// A quad the rasterizer would draw: its footprint, or `drawn` false when it faces away, lies
/// off the screen or covers no pixel centre. Each of those drops only what would draw nothing.
struct QuadView {
    drawn: bool,
    footprint: Footprint,
}

fn view_quad(quad: u32, slot: u32) -> QuadView {
    let desc = sections[slot];
    let origin = section_origin(desc);
    let scale = f32(desc.scale);
    var corners: array<vec3<f32>, CORNERS_PER_QUAD>;
    if ((params.flags & PARAMS_MODEL) != 0u) {
        for (var k = 0u; k < CORNERS_PER_QUAD; k++) {
            let base = (quad * CORNERS_PER_QUAD + k) * MODEL_WORDS_PER_VERTEX;
            corners[k] = origin + model_position(
                model_words[base + MODEL_X_WORD],
                model_words[base + MODEL_Y_WORD],
                model_words[base + MODEL_Z_WORD],
            ) * scale;
        }
    } else {
        var words: array<u32, QUAD_WORDS>;
        for (var word = 0u; word < QUAD_WORDS; word++) {
            words[word] = quad_words[quad * QUAD_WORDS + word];
        }
        for (var k = 0u; k < CORNERS_PER_QUAD; k++) {
            corners[k] = greedy_corner(words, origin, scale, k).world;
        }
    }

    var out: QuadView;
    // The same front the rasterizer keeps: counter-clockwise over corners 1, 2, 0.
    let normal = cross(corners[2] - corners[1], corners[0] - corners[1]);
    if (dot(normal, camera.offset - corners[1]) <= 0.0) {
        out.drawn = false;
        return out;
    }
    var footprint = empty_footprint();
    var projected: array<vec3<f32>, CORNERS_PER_QUAD>;
    for (var k = 0u; k < CORNERS_PER_QUAD; k++) {
        let clip = camera.clip_from_relative * vec4<f32>(corners[k], 1.0);
        footprint.in_front = footprint.in_front && clip.w > 0.0;
        projected[k] = clip.xyz / clip.w;
        footprint.lo = min(footprint.lo, projected[k].xy);
        footprint.hi = max(footprint.hi, projected[k].xy);
        footprint.nearest = max(footprint.nearest, projected[k].z);
    }
    // Models are drawn nudged towards the camera, and the pyramid holds them there, so a model
    // quad is tested as near as the rasterizer may have put it or it would hide behind itself.
    // Anything else is allowed the rounding a corner may land off by.
    var nudge = 4.0 * DEPTH_STEP * footprint.nearest;
    if (footprint.in_front && (params.flags & PARAMS_MODEL) != 0u) {
        let slope = max(
            depth_slope(projected[1], projected[2], projected[0]),
            depth_slope(projected[0], projected[2], projected[3]),
        );
        nudge += bitcast<f32>(MODEL_BIAS_SLOPE_BITS) * slope
            + 2.0 * MODEL_BIAS_CONSTANT * DEPTH_STEP * footprint.nearest;
    }
    footprint.nearest += nudge;
    out.footprint = footprint;
    if (!footprint.in_front) {
        // Crossing the near plane: too close to be worth anything but drawing.
        out.drawn = true;
        return out;
    }
    if (any(footprint.hi < vec2<f32>(-1.0)) || any(footprint.lo > vec2<f32>(1.0))) {
        out.drawn = false;
        return out;
    }
    // A box that holds no pixel centre holds none of the quad inside it either. The margin
    // stands for the rasterizer's snapping of vertices to a fraction of a pixel.
    let margin = 1.0 / 64.0;
    let px_lo = (footprint.lo * 0.5 + 0.5) * camera.viewport - 0.5 - margin;
    let px_hi = (footprint.hi * 0.5 + 0.5) * camera.viewport - 0.5 + margin;
    out.drawn = all(ceil(px_lo) <= floor(px_hi));
    return out;
}

const CULLED: u32 = 0u;
const DRAWN: u32 = 1u;
const HIDDEN: u32 = 2u;

fn first_fate(quad: u32, slot: u32) -> u32 {
    let view = view_quad(quad, slot);
    if (!view.drawn) {
        return CULLED;
    }
    if (hidden_in(hiz, view.footprint)) {
        return HIDDEN;
    }
    return DRAWN;
}

fn second_fate(quad: u32, slot: u32) -> u32 {
    let view = view_quad(quad, slot);
    if (!view.drawn || hidden_in(hiz, view.footprint)) {
        return CULLED;
    }
    return DRAWN;
}

fn every_quad(count: u32) -> u32 {
    return select((1u << count) - 1u, 0xffffffffu, count >= 32u);
}

/// Loads this lane's group of the batch starting at `first` and answers whether it is drawn.
/// A group the last frame's depth alone hides is left to the second pass, and counted against
/// the draw's counter when asked.
fn load(first: u32, local: u32, tally: bool) -> bool {
    let slot = first + local;
    if (slot >= params.group_count) {
        return false;
    }
    let g = groups[params.group_base + slot];
    batch[local] = g;
    candidates[left_word(slot)] = 0u;
    if (g.quad_count == 0u || !section_shown(g.section)) {
        return false;
    }
    let bounds = group_box(g);
    if (!survives(g, bounds)) {
        return false;
    }
    if (hidden_in(hiz, box_footprint(bounds.mn, bounds.mx))) {
        candidates[left_word(slot)] = every_quad(g.quad_count);
        if (tally) {
            atomicAdd(&args[params.counter].vertex_count, g.quad_count);
            let at = append(candidates_counted(), 1u, candidate_dispatch(), GROUP_THREADS);
            candidates[3u * (params.group_base + at) + 2u] = slot;
        }
        return false;
    }
    return true;
}

/// Where a draw's per-pass counts sit in the args, past the draws and the hidden-quad counters:
/// the groups each pass's group cull kept, the groups the first pass left quads of, and the
/// workgroups the kernels reading those lists dispatch.
fn survivors_counted(second: bool) -> u32 {
    return select(3u, 4u, second) * STREAMS + params.args_index;
}

fn candidates_counted() -> u32 {
    return 5u * STREAMS + params.args_index;
}

fn quad_dispatch(second: bool) -> u32 {
    return select(6u, 7u, second) * STREAMS + params.args_index;
}

fn candidate_dispatch() -> u32 {
    return 8u * STREAMS + params.args_index;
}

/// A group's three words: the quads the first pass left to the second, then its place in the
/// list of groups a pass's group cull kept, then its place in the list of groups the first pass
/// left quads of. Both lists start at the draw's first group.
fn left_word(slot: u32) -> u32 {
    return 3u * (params.group_base + slot);
}

fn survivor(index: u32) -> u32 {
    return candidates[3u * (params.group_base + index) + 1u];
}

fn candidate(index: u32) -> u32 {
    return candidates[3u * (params.group_base + index) + 2u];
}

/// Appends `count` entries to the list counted at `counter`, growing the dispatch at `dispatch`
/// to one workgroup every `per_workgroup` entries, and answers where the first one goes.
fn append(counter: u32, count: u32, dispatch: u32, per_workgroup: u32) -> u32 {
    let at = atomicAdd(&args[counter].vertex_count, count);
    let workgroups = min((at + count + per_workgroup - 1u) / per_workgroup, MAX_WORKGROUPS);
    atomicMax(&args[dispatch].vertex_count, workgroups);
    return at;
}

/// Entries a workgroup found this step, by list.
const SURVIVORS: u32 = 0u;
const CANDIDATES: u32 = 1u;
const KEPT: u32 = 2u;
var<workgroup> found: array<atomic<u32>, 3>;

/// Where a lane's entry goes among the workgroup's, counted at `found[counter]`. Every lane of the
/// workgroup has to call it. A subgroup sums its own lanes and takes its run with one atomic,
/// where lanes going one at a time would queue on the counter.
fn place(flag: bool, counter: u32) -> u32 {
#ifdef SUBGROUPS
    let below = subgroupExclusiveAdd(u32(flag));
    let here = subgroupAdd(u32(flag));
    // The first flagged lane takes the run; every other lane holds zero, so the max is its answer.
    var base = 0u;
    if (flag && below == 0u) {
        base = atomicAdd(&found[counter], here);
    }
    return subgroupMax(base) + below;
#else
    var at = 0u;
    if (flag) {
        at = atomicAdd(&found[counter], 1u);
    }
    return at;
#endif
}


var<workgroup> hidden_found: atomic<u32>;
var<workgroup> survivors_base: u32;
var<workgroup> candidates_base: u32;
var<workgroup> list_total: u32;

/// Packs the lanes' survivors and candidates into their lists with one atomic each.
fn publish(local: u32, second: bool, survivor_slot: u32, lives: bool, candidate_slot: u32, left: bool) {
    let at = place(lives, SURVIVORS);
    let left_at = place(left, CANDIDATES);
    workgroupBarrier();
    if (local == 0u) {
        let kept = atomicLoad(&found[SURVIVORS]);
        if (kept != 0u) {
            survivors_base = append(survivors_counted(second), kept, quad_dispatch(second), GROUPS_PER_STEP);
        }
        let parked = atomicLoad(&found[CANDIDATES]);
        if (parked != 0u) {
            candidates_base = append(candidates_counted(), parked, candidate_dispatch(), GROUP_THREADS);
        }
        let hidden = atomicLoad(&hidden_found);
        if (hidden != 0u) {
            atomicAdd(&args[params.counter].vertex_count, hidden);
        }
        atomicStore(&found[SURVIVORS], 0u);
        atomicStore(&found[CANDIDATES], 0u);
        atomicStore(&hidden_found, 0u);
    }
    let base = workgroupUniformLoad(&survivors_base);
    let left_base = workgroupUniformLoad(&candidates_base);
    if (lives) {
        candidates[3u * (params.group_base + base + at) + 1u] = survivor_slot;
    }
    if (left) {
        candidates[3u * (params.group_base + left_base + left_at) + 2u] = candidate_slot;
    }
}

/// One lane a group, and nothing held across the test, so a group the section bit or the frustum
/// drops costs little more than reading its record.
fn group_cull(workgroup: u32, grid: u32, local: u32) {
    for (var first = workgroup * GROUP_THREADS; first < params.group_count; first += grid * GROUP_THREADS) {
        let slot = first + local;
        var lives = false;
        var hidden = false;
        // A group's word is read only through the list of groups left to the second pass, so
        // only a group going on that list needs it written.
        if (slot < params.group_count) {
            let g = groups[params.group_base + slot];
            if (g.quad_count != 0u && section_shown(g.section)) {
                let bounds = group_box(g);
                if (survives(g, bounds)) {
                    if (hidden_in(hiz, box_footprint(bounds.mn, bounds.mx))) {
                        candidates[left_word(slot)] = every_quad(g.quad_count);
                        hidden = true;
                        atomicAdd(&hidden_found, g.quad_count);
                    } else {
                        lives = true;
                    }
                }
            }
        }
        publish(local, false, slot, lives, slot, hidden);
    }
}

/// The groups the first pass left quads of, tested again against the depth this frame has drawn
/// so far.
fn group_cull_second(workgroup: u32, grid: u32, local: u32) {
    if (local == 0u) {
        list_total = atomicLoad(&args[candidates_counted()].vertex_count);
    }
    let total = workgroupUniformLoad(&list_total);
    for (var first = workgroup * GROUP_THREADS; first < total; first += grid * GROUP_THREADS) {
        let index = first + local;
        var lives = false;
        var slot = 0u;
        if (index < total) {
            slot = candidate(index);
            let bounds = group_box(groups[params.group_base + slot]);
            lives = !hidden_in(hiz, box_footprint(bounds.mn, bounds.mx));
        }
        publish(local, true, slot, lives, 0u, false);
    }
}

/// Groups a quad workgroup takes at once, one to each run of `CULL_THREADS` lanes, so the draw's
/// counter is contended once for this many groups.
const GROUPS_PER_STEP: u32 = #{GROUPS_PER_STEP}u;
const QUAD_THREADS: u32 = CULL_THREADS * GROUPS_PER_STEP;

var<workgroup> step_left: array<atomic<u32>, GROUPS_PER_STEP>;
var<workgroup> survivors_total: u32;

/// The quads of the groups the group cull kept. A group holds no more quads than a run has
/// lanes, so each lane tests its own quad of its run's group. The second pass tests only the
/// quads the first left, and packs what it draws after the first pass's quads.
fn quad_cull(workgroup: u32, grid: u32, local: u32, second: bool) {
    let tested = (params.flags & PARAMS_QUAD_CULL) != 0u && camera.quad_cull != 0u;
    if (local == 0u) {
        survivors_total = atomicLoad(&args[survivors_counted(second)].vertex_count);
    }
    let survivors = workgroupUniformLoad(&survivors_total);
    var args_slot = params.args_index;
    var offset = 0u;
    if (second) {
        offset = atomicLoad(&args[params.args_index].vertex_count) / VERTICES_PER_QUAD;
        args_slot = params.args_index + STREAMS;
        if (workgroup == 0u && local == 0u) {
            args[args_slot].first_vertex = offset * VERTICES_PER_QUAD;
        }
    }
    let lane = local % CULL_THREADS;
    let run = local / CULL_THREADS;
    let bit = 1u << lane;
    for (var first = workgroup * GROUPS_PER_STEP; first < survivors; first += grid * GROUPS_PER_STEP) {
        if (local < GROUPS_PER_STEP) {
            atomicStore(&step_left[local], 0u);
        }
        workgroupBarrier();
        let index = first + run;
        var slot = 0u;
        var keep = false;
        var entry = vec2<u32>(0u);
        if (index < survivors) {
            slot = survivor(index);
            let g = groups[params.group_base + slot];
            let asked = !second || (candidates[left_word(slot)] & bit) != 0u;
            if (lane < g.quad_count && asked) {
                let quad = g.quad_base + lane;
                var fate = DRAWN;
                if (tested) {
                    if (second) {
                        fate = second_fate(quad, g.section);
                    } else {
                        fate = first_fate(quad, g.section);
                    }
                }
                keep = fate == DRAWN;
                entry = vec2<u32>(quad, g.section);
                if (fate == HIDDEN) {
                    atomicOr(&step_left[run], bit);
                }
            }
        }
        let at = place(keep, KEPT);
        workgroupBarrier();
        if (local == 0u) {
            let total = atomicLoad(&found[KEPT]);
            reserved = offset;
            if (total != 0u) {
                reserved = offset
                    + atomicAdd(&args[args_slot].vertex_count, total * VERTICES_PER_QUAD)
                        / VERTICES_PER_QUAD;
            }
            atomicStore(&found[KEPT], 0u);
            if (!second) {
                var hidden = 0u;
                for (var k = 0u; k < GROUPS_PER_STEP; k++) {
                    hidden += countOneBits(atomicLoad(&step_left[k]));
                }
                if (hidden != 0u) {
                    atomicAdd(&args[params.counter].vertex_count, hidden);
                }
            }
        }
        var parked = false;
        if (!second && lane == 0u && index < survivors) {
            let left = atomicLoad(&step_left[run]);
            if (left != 0u) {
                candidates[left_word(slot)] = left;
                parked = true;
            }
        }
        let base = params.visible_base + workgroupUniformLoad(&reserved);
        if (keep) {
            visible[base + at] = entry;
        }
        if (!second) {
            publish(local, false, 0u, false, slot, parked);
        }
    }
}

@compute @workgroup_size(GROUP_THREADS)
fn cull_groups(
    @builtin(workgroup_id) workgroup: vec3<u32>,
    @builtin(num_workgroups) grid: vec3<u32>,
    @builtin(local_invocation_index) local: u32,
) {
    group_cull(workgroup.x, grid.x, local);
}

@compute @workgroup_size(GROUP_THREADS)
fn cull_groups_second(
    @builtin(workgroup_id) workgroup: vec3<u32>,
    @builtin(num_workgroups) grid: vec3<u32>,
    @builtin(local_invocation_index) local: u32,
) {
    group_cull_second(workgroup.x, grid.x, local);
}

@compute @workgroup_size(QUAD_THREADS)
fn cull_quads(
    @builtin(workgroup_id) workgroup: vec3<u32>,
    @builtin(num_workgroups) grid: vec3<u32>,
    @builtin(local_invocation_index) local: u32,
) {
    quad_cull(workgroup.x, grid.x, local, false);
}

@compute @workgroup_size(QUAD_THREADS)
fn cull_quads_second(
    @builtin(workgroup_id) workgroup: vec3<u32>,
    @builtin(num_workgroups) grid: vec3<u32>,
    @builtin(local_invocation_index) local: u32,
) {
    quad_cull(workgroup.x, grid.x, local, true);
}

/// Blended geometry keeps the back-to-front order the blend depends on, so its survivors are
/// packed by a prefix sum over the batches rather than by an atomic: `count_ordered` writes each
/// batch's surviving quads, `scan_ordered` turns them into where each batch starts, and
/// `scatter_ordered` writes the quads there. A batch's index is its first group's slot in the
/// arena over `CULL_THREADS`, which a stream's block, aligned to a power of two, keeps exact.
@compute @workgroup_size(CULL_THREADS)
fn count_ordered(
    @builtin(workgroup_id) workgroup: vec3<u32>,
    @builtin(num_workgroups) grid: vec3<u32>,
    @builtin(local_invocation_index) local: u32,
) {
    var first = workgroup.x * CULL_THREADS;
    loop {
        if (first >= params.group_count) {
            break;
        }
        let lives = load(first, local, true);
        counts[local] = select(0u, batch[local].quad_count, lives);
        workgroupBarrier();
        if (local == 0u) {
            var total = 0u;
            for (var k = 0u; k < CULL_THREADS; k = k + 1u) {
                total = total + counts[k];
            }
            batches[(params.group_base + first) / CULL_THREADS] = total;
        }
        workgroupBarrier();
        first = first + grid.x * CULL_THREADS;
    }
}

var<workgroup> partial: array<u32, CULL_THREADS>;

/// One workgroup per ordered draw: each lane scans a run of its batches, the runs are joined,
/// and the draw's instance count becomes what survived in all.
@compute @workgroup_size(CULL_THREADS)
fn scan_ordered(@builtin(local_invocation_index) local: u32) {
    let first = params.group_base / CULL_THREADS;
    let count = (params.group_count + CULL_THREADS - 1u) / CULL_THREADS;
    let run = (count + CULL_THREADS - 1u) / CULL_THREADS;
    let lo = min(local * run, count);
    let hi = min(lo + run, count);
    var sum = 0u;
    for (var b = lo; b < hi; b = b + 1u) {
        sum = sum + batches[first + b];
    }
    partial[local] = sum;
    workgroupBarrier();
    var running = 0u;
    for (var k = 0u; k < local; k = k + 1u) {
        running = running + partial[k];
    }
    for (var b = lo; b < hi; b = b + 1u) {
        let here = batches[first + b];
        batches[first + b] = running;
        running = running + here;
    }
    if (local == CULL_THREADS - 1u) {
        atomicStore(&args[params.args_index].vertex_count, running * VERTICES_PER_QUAD);
    }
}

@compute @workgroup_size(CULL_THREADS)
fn scatter_ordered(
    @builtin(workgroup_id) workgroup: vec3<u32>,
    @builtin(num_workgroups) grid: vec3<u32>,
    @builtin(local_invocation_index) local: u32,
) {
    var first = workgroup.x * CULL_THREADS;
    loop {
        if (first >= params.group_count) {
            break;
        }
        let lives = load(first, local, false);
        counts[local] = select(0u, batch[local].quad_count, lives);
        workgroupBarrier();
        if (local == 0u) {
            var total = batches[(params.group_base + first) / CULL_THREADS];
            for (var k = 0u; k < CULL_THREADS; k = k + 1u) {
                starts[k] = total;
                total = total + counts[k];
            }
        }
        workgroupBarrier();

        let in_batch = min(CULL_THREADS, params.group_count - first);
        for (var k = 0u; k < in_batch; k = k + 1u) {
            if (counts[k] == 0u) {
                continue;
            }
            let g = batch[k];
            var i = local;
            loop {
                if (i >= g.quad_count) {
                    break;
                }
                visible[params.visible_base + starts[k] + i] = vec2<u32>(g.quad_base + i, g.section);
                i = i + CULL_THREADS;
            }
        }
        workgroupBarrier();
        first = first + grid.x * CULL_THREADS;
    }
}
