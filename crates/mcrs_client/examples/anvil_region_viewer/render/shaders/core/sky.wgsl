
#import anvil_region_viewer::frame::{sky, view}
#import anvil_region_viewer::region::degenerate
#import anvil_region_viewer::sky_bindings::{celestial_sampler, celestials}

const PI: f32 = 3.14159265359;

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
