#import bevy_render::view::View
#import anvil_region_viewer::layout::{Sky, degenerate}

const PI: f32 = 3.14159265359;

@group(0) @binding(0) var<uniform> view: View;
@group(0) @binding(3) var<uniform> sky: Sky;

@group(1) @binding(0) var celestials: texture_2d_array<f32>;
@group(1) @binding(1) var celestial_sampler: sampler;
@group(1) @binding(2) var clouds: texture_2d_array<f32>;

const SKY_DISC_Y: f32 = 16.0;
const DARK_DISC_Y: f32 = -4.0;
const DISC_VERTICES: u32 = 24u;

const SUNRISE_STEPS: u32 = 16u;
const SUNRISE_RADIUS: f32 = 120.0;
const SUNRISE_LEAN: f32 = 40.0;

const CELESTIAL_HEIGHT: f32 = 100.0;
const SUN_SIZE: f32 = 30.0;
const MOON_SIZE: f32 = 20.0;

const STAR_DISTANCE: f32 = 100.0;

const CLOUD_CELL: f32 = 12.0;
const CLOUD_THICKNESS: f32 = 4.0;
const CLOUD_OPEN: f32 = 10.0 / 255.0;
const CLOUD_STEPS: u32 = 192u;
const CLOUD_TOP: f32 = 1.0;
const CLOUD_BOTTOM: f32 = 0.7;
const CLOUD_SIDE_X: f32 = 0.9;
const CLOUD_SIDE_Z: f32 = 0.8;

struct SkyVertex {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) @interpolate(flat) layer: u32,
    @location(3) distance: f32,
};

fn to_clip(local: vec3<f32>) -> vec4<f32> {
    return view.clip_from_world * vec4<f32>(view.world_position.xyz + local, 1.0);
}

fn rotate_x(v: vec3<f32>, angle: f32) -> vec3<f32> {
    let s = sin(angle);
    let c = cos(angle);
    return vec3<f32>(v.x, v.y * c - v.z * s, v.y * s + v.z * c);
}

fn rotate_y(v: vec3<f32>, angle: f32) -> vec3<f32> {
    let s = sin(angle);
    let c = cos(angle);
    return vec3<f32>(v.x * c + v.z * s, v.y, -v.x * s + v.z * c);
}

fn rotate_z(v: vec3<f32>, angle: f32) -> vec3<f32> {
    let s = sin(angle);
    let c = cos(angle);
    return vec3<f32>(v.x * c - v.y * s, v.x * s + v.y * c, v.z);
}

fn celestial_frame(local: vec3<f32>, angle: f32) -> vec3<f32> {
    return rotate_y(rotate_x(local, angle), -PI / 2.0);
}

fn disc_corner(index: u32, y: f32) -> vec3<f32> {
    let corner = index % 3u;
    if corner == 0u {
        return vec3<f32>(0.0, y, 0.0);
    }
    let step = index / 3u + corner - 1u;
    let angle = (-180.0 + f32(step) * 45.0) * PI / 180.0;
    let radius = sky.fog.a;
    return vec3<f32>(sign(y) * radius * cos(angle), y, radius * sin(angle));
}

@vertex
fn vertex_disc(@builtin(vertex_index) index: u32) -> SkyVertex {
    var out: SkyVertex;
    out.uv = vec2<f32>(0.0);
    out.layer = 0u;
    var local = vec3<f32>(0.0);
    if index < DISC_VERTICES {
        local = disc_corner(index, SKY_DISC_Y);
        out.color = vec4<f32>(sky.disc.rgb, 1.0);
        out.clip_position = to_clip(local);
    } else if sky.disc.a > 0.5 {
        local = disc_corner(index - DISC_VERTICES, DARK_DISC_Y);
        out.color = vec4<f32>(0.0, 0.0, 0.0, 1.0);
        out.clip_position = to_clip(local);
    } else {
        out.color = vec4<f32>(0.0);
        out.clip_position = degenerate();
    }
    out.distance = length(local);
    return out;
}

@vertex
fn vertex_sunrise(@builtin(vertex_index) index: u32) -> SkyVertex {
    var out: SkyVertex;
    out.uv = vec2<f32>(0.0);
    out.layer = 0u;
    out.distance = 0.0;
    let alpha = sky.sunrise.a;
    if alpha <= 0.001 {
        out.clip_position = degenerate();
        out.color = vec4<f32>(0.0);
        return out;
    }

    let corner = index % 3u;
    var local = vec3<f32>(0.0, CELESTIAL_HEIGHT, 0.0);
    var fade = 1.0;
    if corner != 0u {
        let step = index / 3u + corner - 1u;
        let angle = f32(step) * 2.0 * PI / f32(SUNRISE_STEPS);
        local = vec3<f32>(
            sin(angle) * SUNRISE_RADIUS,
            cos(angle) * SUNRISE_RADIUS,
            -cos(angle) * SUNRISE_LEAN,
        );
        fade = 0.0;
    }
    local.z *= alpha;
    let side = select(0.0, PI, sin(sky.angles.x) < 0.0);
    out.clip_position = to_clip(rotate_x(rotate_z(local, side + PI / 2.0), PI / 2.0));
    out.color = vec4<f32>(sky.sunrise.rgb, alpha * fade);
    return out;
}

fn quad_corner(vertex: u32) -> u32 {
    switch vertex {
        case 0u, 3u: { return 0u; }
        case 1u: { return 1u; }
        case 2u, 4u: { return 2u; }
        default: { return 3u; }
    }
}

@vertex
fn vertex_celestial(@builtin(vertex_index) index: u32) -> SkyVertex {
    let corner = quad_corner(index % 6u);
    let sign_x = select(-1.0, 1.0, corner == 1u || corner == 2u);
    let sign_z = select(-1.0, 1.0, corner >= 2u);

    let is_moon = index >= 6u;
    let size = select(SUN_SIZE, MOON_SIZE, is_moon);
    let angle = sky.angles.x + select(0.0, PI, is_moon);
    let local = vec3<f32>(sign_x * size, CELESTIAL_HEIGHT, sign_z * size);

    var uv = vec2<f32>(sign_x, sign_z) * 0.5 + 0.5;
    if is_moon {
        uv = 1.0 - uv;
    }

    var out: SkyVertex;
    out.clip_position = to_clip(celestial_frame(local, angle));
    out.color = vec4<f32>(1.0, 1.0, 1.0, sky.moon.y);
    out.uv = uv;
    out.distance = 0.0;
    out.layer = select(0u, u32(sky.moon.x), is_moon);
    return out;
}

fn hash(value: u32) -> u32 {
    var state = value * 747796405u + 2891336453u;
    state = ((state >> ((state >> 28u) + 4u)) ^ state) * 277803737u;
    return (state >> 22u) ^ state;
}

fn random(index: u32) -> f32 {
    return f32(hash(index)) / 4294967296.0;
}

@vertex
fn vertex_stars(@builtin(vertex_index) index: u32) -> SkyVertex {
    let star = index / 6u;
    let draw = star * 5u;
    let point = vec3<f32>(
        random(draw) * 2.0 - 1.0,
        random(draw + 1u) * 2.0 - 1.0,
        random(draw + 2u) * 2.0 - 1.0,
    );
    let size = 0.15 + random(draw + 3u) * 0.1;
    let spin = random(draw + 4u) * 2.0 * PI;

    var out: SkyVertex;
    out.uv = vec2<f32>(0.0);
    out.layer = 0u;
    out.distance = 0.0;
    out.color = vec4<f32>(sky.angles.y);

    let length_squared = dot(point, point);
    if length_squared <= 0.010000001 || length_squared >= 1.0 {
        out.clip_position = degenerate();
        return out;
    }

    let facing = -normalize(point);
    let centre = -facing * STAR_DISTANCE;
    let left = normalize(cross(vec3<f32>(0.0, 1.0, 0.0), facing));
    let up = cross(facing, left);

    let corner = quad_corner(index % 6u);
    let flat = vec2<f32>(
        select(-size, size, corner <= 1u),
        select(-size, size, corner == 1u || corner == 2u),
    );
    let turned = rotate_z(vec3<f32>(flat, 0.0), -spin);
    let local = centre + left * turned.x + up * turned.y;
    out.clip_position = to_clip(celestial_frame(local, sky.angles.x));
    return out;
}

@fragment
fn fragment_flat(in: SkyVertex) -> @location(0) vec4<f32> {
    return in.color;
}

@fragment
fn fragment_disc(in: SkyVertex) -> @location(0) vec4<f32> {
    let fog = clamp(in.distance / sky.fog.a, 0.0, 1.0);
    return vec4<f32>(mix(in.color.rgb, sky.fog.rgb, fog), in.color.a);
}

@fragment
fn fragment_celestial(in: SkyVertex) -> @location(0) vec4<f32> {
    return textureSampleLevel(celestials, celestial_sampler, in.uv, in.layer, 0.0) * in.color;
}


struct CloudVertex {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) ndc: vec2<f32>,
};

struct CloudFragment {
    @location(0) color: vec4<f32>,
    @builtin(frag_depth) depth: f32,
};

@vertex
fn vertex_clouds(@builtin(vertex_index) index: u32) -> CloudVertex {
    let corner = vec2<f32>(f32((index << 1u) & 2u), f32(index & 2u));
    var out: CloudVertex;
    out.ndc = corner * 2.0 - 1.0;
    out.clip_position = vec4<f32>(out.ndc, 1.0, 1.0);
    return out;
}

fn cloud_cell(cell: vec2<i32>, size: vec2<i32>) -> vec4<f32> {
    return textureLoad(clouds, cell & (size - vec2<i32>(1)), 0, 0);
}

@fragment
fn fragment_clouds(in: CloudVertex) -> CloudFragment {
    var out: CloudFragment;
    out.color = vec4<f32>(0.0);
    out.depth = 0.0;

    let origin = view.world_position.xyz;
    let near = view.world_from_clip * vec4<f32>(in.ndc, 1.0, 1.0);
    let direction = normalize(near.xyz / near.w - origin);

    let bottom = sky.cloud.x;
    let top = bottom + CLOUD_THICKNESS;
    let fade = sky.cloud.w;

    var enter = 0.0;
    var leave = fade;
    if abs(direction.y) < 1e-6 {
        if origin.y < bottom || origin.y > top {
            discard;
        }
    } else {
        let first = (bottom - origin.y) / direction.y;
        let second = (top - origin.y) / direction.y;
        enter = max(0.0, min(first, second));
        leave = min(fade, max(first, second));
    }
    if enter > leave {
        discard;
    }

    let size = vec2<i32>(textureDimensions(clouds));
    let span = f32(size.x) * CLOUD_CELL;
    let drift = vec2<f32>(sky.cloud.y % span, sky.cloud.z);
    let start = (origin.xz + drift + direction.xz * enter) / CLOUD_CELL;

    let corner = floor(start);
    let step = sign(direction.xz);
    let per_cell = CLOUD_CELL / max(abs(direction.xz), vec2<f32>(1e-6));
    var next = enter
        + select(start - corner, corner + 1.0 - start, step > vec2<f32>(0.0)) * per_cell;
    var cell = vec2<i32>(corner);
    let cell_step = vec2<i32>(step);

    var shade = select(CLOUD_BOTTOM, CLOUD_TOP, direction.y < 0.0);
    var distance = enter;
    var filled = vec4<f32>(0.0);
    for (var taken = 0u; taken < CLOUD_STEPS; taken = taken + 1u) {
        let sample = cloud_cell(cell, size);
        if sample.a >= CLOUD_OPEN {
            filled = sample;
            break;
        }
        if next.x < next.y {
            distance = next.x;
            next.x = next.x + per_cell.x;
            cell.x = cell.x + cell_step.x;
            shade = CLOUD_SIDE_X;
        } else {
            distance = next.y;
            next.y = next.y + per_cell.y;
            cell.y = cell.y + cell_step.y;
            shade = CLOUD_SIDE_Z;
        }
        if distance > leave {
            break;
        }
    }
    if filled.a < CLOUD_OPEN {
        discard;
    }

    let position = origin + direction * distance;
    let clip = view.clip_from_world * vec4<f32>(position, 1.0);
    out.depth = clip.z / clip.w;
    out.color = vec4<f32>(
        filled.rgb * sky.cloud_color.rgb * shade,
        filled.a * sky.cloud_color.a * (1.0 - clamp(distance / fade, 0.0, 1.0)),
    );
    return out;
}
