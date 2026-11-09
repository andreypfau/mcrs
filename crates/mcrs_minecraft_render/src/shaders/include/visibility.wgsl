#define_import_path mcrs_minecraft_client::visibility

// A frustum plane is `(normal, d)`, with the inside where `dot(normal, p) + d > 0`.

fn box_outside(plane: vec4<f32>, mn: vec3<f32>, mx: vec3<f32>) -> bool {
    let corner = vec3<f32>(
        select(mn.x, mx.x, plane.x > 0.0),
        select(mn.y, mx.y, plane.y > 0.0),
        select(mn.z, mx.z, plane.z > 0.0),
    );
    return dot(plane.xyz, corner) + plane.w <= 0.0;
}

fn sphere_outside(plane: vec4<f32>, centre: vec3<f32>, radius: f32) -> bool {
    return dot(plane.xyz, centre) + plane.w <= -radius;
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

fn cover(footprint: ptr<function, Footprint>, clip_from_relative: mat4x4<f32>, corner: vec3<f32>) {
    let clip = clip_from_relative * vec4<f32>(corner, 1.0);
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
fn farthest_under(
    pyramid: texture_2d<f32>,
    viewport: vec2<f32>,
    levels: u32,
    lo: vec2<f32>,
    hi: vec2<f32>,
) -> f32 {
    let uv_lo = vec2<f32>(lo.x, -hi.y) * 0.5 + 0.5;
    let uv_hi = vec2<f32>(hi.x, -lo.y) * 0.5 + 0.5;
    let pixels = vec2<u32>(viewport);
    let base = textureDimensions(pyramid, 0);
    let b_lo = min(min(vec2<u32>(uv_lo * viewport), pixels - 1u) >> vec2<u32>(1u), base - 1u);
    let b_hi = min(min(vec2<u32>(uv_hi * viewport), pixels - 1u) >> vec2<u32>(1u), base - 1u);
    let extent = b_hi - b_lo + 1u;
    let level = min(u32(ceil(log2(f32(max(extent.x, extent.y))))), levels - 1u);
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

/// Whether a pyramid of at least one level holds something nearer than every point of the
/// footprint over the whole of it. A shape reaching behind the camera or past the edge of the
/// screen is never hidden: what the pyramid does not cover says nothing about it.
fn occluded(pyramid: texture_2d<f32>, viewport: vec2<f32>, levels: u32, footprint: Footprint) -> bool {
    if (!footprint.in_front) {
        return false;
    }
    if (any(footprint.lo < vec2<f32>(-1.0)) || any(footprint.hi > vec2<f32>(1.0))) {
        return false;
    }
    return footprint.nearest < farthest_under(pyramid, viewport, levels, footprint.lo, footprint.hi);
}
