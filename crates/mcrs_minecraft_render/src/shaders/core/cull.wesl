
import mcrs_minecraft_render::shaders::include::cull_layout::{
    DRAW_KINDS, ROW_CANDIDATES, ROW_CANDIDATE_DISPATCH, ROW_FIRST_PASS_DRAWS,
    ROW_FIRST_PASS_QUAD_DISPATCH, ROW_FIRST_PASS_SURVIVORS, ROW_SECOND_PASS_DRAWS,
    ROW_SECOND_PASS_QUAD_DISPATCH, ROW_SECOND_PASS_SURVIVORS, args, batches, candidates,
    cave_visible, groups, hiz, model_words, quad_words, sections, visible,
};
import mcrs_minecraft_render::shaders::include::fields::{
    BOUNDS_HI_X_BITS, BOUNDS_HI_X_SHIFT, BOUNDS_HI_Y_BITS, BOUNDS_HI_Y_SHIFT,
    BOUNDS_HI_Z_BITS, BOUNDS_HI_Z_SHIFT, BOUNDS_LO_X_BITS, BOUNDS_LO_X_SHIFT,
    BOUNDS_LO_Y_BITS, BOUNDS_LO_Y_SHIFT, BOUNDS_LO_Z_BITS, BOUNDS_LO_Z_SHIFT,
    FACE_NONE, Group, MODEL_OVERHANG, MODEL_X_WORD, MODEL_Y_WORD, MODEL_Z_WORD, QUAD_WORDS,
    SECTION_SIZE,
};
import mcrs_minecraft_render::shaders::include::frame::{PARAMS_MODEL, PARAMS_QUAD_CULL, camera, params};
import mcrs_minecraft_render::shaders::include::quad::{
    CORNERS_PER_QUAD, GREEDY_EDGE_OVERLAP, MODEL_WORDS_PER_VERTEX, corner_uv, expanded_quad_uv,
    face_normal, greedy_corner, model_position,
};
import mcrs_minecraft_render::shaders::include::section::section_origin;
import mcrs_minecraft_render::shaders::include::section_layout::SectionDesc;
import mcrs_minecraft_render::shaders::include::visibility::{
    Footprint, box_outside, cover, empty_footprint, occluded,
};

const VERTICES_PER_QUAD: u32 = 6u;

const CULL_THREADS: u32 = constants::CULL_THREADS;
const GROUP_THREADS: u32 = constants::GROUP_THREADS;
const CAVE_WORDS: u32 = constants::CAVE_WORDS;
const SECTION_SLOTS: u32 = constants::SECTION_SLOTS;
const MAX_WORKGROUPS: u32 = constants::MAX_WORKGROUPS;
const NO_SLOT: u32 = 0xffffffffu;


var<workgroup> batch: array<Group, CULL_THREADS>;
var<workgroup> counts: array<u32, CULL_THREADS>;
var<workgroup> starts: array<u32, CULL_THREADS>;
var<workgroup> reserved: u32;
var<workgroup> shown_word: atomic<u32>;

fn in_frustum(mn: vec3<f32>, mx: vec3<f32>) -> bool {
    for (var i = 0u; i < 5u; i = i + 1u) {
        if (box_outside(camera.frustum[i], mn, mx)) {
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
    let margin = vec3<f32>(GREEDY_EDGE_OVERLAP * scale);
    return Box(origin + lo * scale - margin, origin + hi * scale + margin);
}

fn section_shown(slot: u32) -> bool {
    return ((cave_visible[CAVE_WORDS + (slot >> 5u)] >> (slot & 31u)) & 1u) != 0u;
}

fn survives(g: Group, bounds: Box) -> bool {
    return in_frustum(bounds.mn, bounds.mx) && faces_camera(g.face, bounds.mn, bounds.mx);
}

fn section_box(slot: u32) -> Box {
    let desc = sections[slot];
    let origin = section_origin(desc);
    let span = SECTION_SIZE * f32(desc.scale);
    return Box(origin - MODEL_OVERHANG, origin + span + MODEL_OVERHANG);
}

fn column_drawn(desc: SectionDesc) -> bool {
    let reach = i32(camera.render_distance);
    let away = max(abs(vec2<i32>(desc.x, desc.z) - camera.section.xz) - vec2<i32>(1), vec2<i32>(0));
    return all(away < vec2<i32>(reach)) && dot(away, away) < reach * reach;
}

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
            if (column_drawn(sections[slot]) && in_frustum(bounds.mn, bounds.mx)) {
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

const MODEL_BIAS_CONSTANT: f32 = constants::MODEL_BIAS_CONSTANT;
const MODEL_BIAS_SLOPE_BITS: u32 = constants::MODEL_BIAS_SLOPE_BITS;
const DEPTH_STEP: f32 = 1.0 / 8388608.0;

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

fn hidden_in(pyramid: texture_2d<f32>, footprint: Footprint) -> bool {
    if (camera.hiz_levels == 0u) {
        return false;
    }
    return occluded(pyramid, camera.viewport, camera.hiz_levels, footprint);
}

fn box_footprint(mn: vec3<f32>, mx: vec3<f32>) -> Footprint {
    var footprint = empty_footprint();
    for (var i = 0u; i < 8u; i = i + 1u) {
        cover(&footprint, camera.clip_from_relative, vec3<f32>(
            select(mn.x, mx.x, (i & 1u) != 0u),
            select(mn.y, mx.y, (i & 2u) != 0u),
            select(mn.z, mx.z, (i & 4u) != 0u),
        ));
    }
    return footprint;
}


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
        var local: array<vec3<f32>, CORNERS_PER_QUAD>;
        for (var k = 0u; k < CORNERS_PER_QUAD; k++) {
            let base = (quad * CORNERS_PER_QUAD + k) * MODEL_WORDS_PER_VERTEX;
            local[k] = model_position(
                model_words[base + MODEL_X_WORD],
                model_words[base + MODEL_Y_WORD],
                model_words[base + MODEL_Z_WORD],
            );
        }
        let u = local[3] - local[0];
        let v = local[1] - local[0];
        let is_parallelogram = all(abs(local[0] + u + v - local[2]) < vec3<f32>(1e-4))
            && dot(u, u) > 1e-8
            && dot(v, v) > 1e-8;
        for (var k = 0u; k < CORNERS_PER_QUAD; k++) {
            if (is_parallelogram) {
                let quad_uv = expanded_quad_uv(u, v, corner_uv(k), GREEDY_EDGE_OVERLAP);
                corners[k] = origin + (local[0] + u * quad_uv.x + v * quad_uv.y) * scale;
            } else {
                corners[k] = origin + local[k] * scale;
            }
        }
    } else {
        var words: array<u32, QUAD_WORDS>;
        for (var word = 0u; word < QUAD_WORDS; word++) {
            words[word] = quad_words[quad * QUAD_WORDS + word];
        }
        for (var k = 0u; k < CORNERS_PER_QUAD; k++) {
            corners[k] = greedy_corner(words, origin, scale, k, GREEDY_EDGE_OVERLAP).world;
        }
    }

    var out: QuadView;
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
        out.drawn = true;
        return out;
    }
    if (any(footprint.hi < vec2<f32>(-1.0)) || any(footprint.lo > vec2<f32>(1.0))) {
        out.drawn = false;
        return out;
    }
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

fn survivors_counted(second: bool) -> u32 {
    return select(ROW_FIRST_PASS_SURVIVORS, ROW_SECOND_PASS_SURVIVORS, second) * DRAW_KINDS
        + params.args_index;
}

fn candidates_counted() -> u32 {
    return ROW_CANDIDATES * DRAW_KINDS + params.args_index;
}

fn quad_dispatch(second: bool) -> u32 {
    return select(ROW_FIRST_PASS_QUAD_DISPATCH, ROW_SECOND_PASS_QUAD_DISPATCH, second) * DRAW_KINDS
        + params.args_index;
}

fn candidate_dispatch() -> u32 {
    return ROW_CANDIDATE_DISPATCH * DRAW_KINDS + params.args_index;
}

fn left_word(slot: u32) -> u32 {
    return 3u * (params.group_base + slot);
}

fn survivor(index: u32) -> u32 {
    return candidates[3u * (params.group_base + index) + 1u];
}

fn candidate(index: u32) -> u32 {
    return candidates[3u * (params.group_base + index) + 2u];
}

fn append(counter: u32, count: u32, dispatch: u32, per_workgroup: u32) -> u32 {
    let at = atomicAdd(&args[counter].vertex_count, count);
    let workgroups = min((at + count + per_workgroup - 1u) / per_workgroup, MAX_WORKGROUPS);
    atomicMax(&args[dispatch].vertex_count, workgroups);
    return at;
}

const SURVIVORS: u32 = 0u;
const CANDIDATES: u32 = 1u;
const KEPT: u32 = 2u;
var<workgroup> found: array<atomic<u32>, 3>;

fn place(flag: bool, counter: u32) -> u32 {
    @if(SUBGROUPS) {
        let below = subgroupExclusiveAdd(u32(flag));
        let here = subgroupAdd(u32(flag));
        var base = 0u;
        if (flag && below == 0u) {
            base = atomicAdd(&found[counter], here);
        }
        return subgroupMax(base) + below;
    } @else {
        var at = 0u;
        if (flag) {
            at = atomicAdd(&found[counter], 1u);
        }
        return at;
    }
}


var<workgroup> hidden_found: atomic<u32>;
var<workgroup> survivors_base: u32;
var<workgroup> candidates_base: u32;
var<workgroup> list_total: u32;

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

fn group_cull(workgroup: u32, grid: u32, local: u32) {
    for (var first = workgroup * GROUP_THREADS; first < params.group_count; first += grid * GROUP_THREADS) {
        let slot = first + local;
        var lives = false;
        var hidden = false;
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

const GROUPS_PER_STEP: u32 = constants::GROUPS_PER_STEP;
const QUAD_THREADS: u32 = CULL_THREADS * GROUPS_PER_STEP;

var<workgroup> step_left: array<atomic<u32>, GROUPS_PER_STEP>;
var<workgroup> survivors_total: u32;

fn quad_cull(workgroup: u32, grid: u32, local: u32, second: bool) {
    let tested = (params.flags & PARAMS_QUAD_CULL) != 0u && camera.quad_cull != 0u;
    if (local == 0u) {
        survivors_total = atomicLoad(&args[survivors_counted(second)].vertex_count);
    }
    let survivors = workgroupUniformLoad(&survivors_total);
    var args_slot = ROW_FIRST_PASS_DRAWS * DRAW_KINDS + params.args_index;
    var offset = 0u;
    if (second) {
        offset = atomicLoad(&args[args_slot].vertex_count) / VERTICES_PER_QUAD;
        args_slot = ROW_SECOND_PASS_DRAWS * DRAW_KINDS + params.args_index;
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
        atomicStore(&args[ROW_FIRST_PASS_DRAWS * DRAW_KINDS + params.args_index].vertex_count, running * VERTICES_PER_QUAD);
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
