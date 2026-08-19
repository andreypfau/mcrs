// The sky, drawn the way `SkyRenderer` draws it and in the same order: the sky disc, the twilight
// band, the sun and the moon, then the stars.
//
// Vanilla builds every one of those on the CPU into a vertex buffer — a ten-vertex fan for the
// disc, an eighteen-vertex fan for the band, a quad per celestial body, and fifteen hundred quads
// of stars laid out once at start-up. Here there is no buffer and nothing to lay out: each vertex
// is derived from its own index, and the star field is generated in the shader from a hash rather
// than uploaded.
//
// The whole thing is drawn before the terrain with the depth test off and no depth write, which is
// what the sky pass does in vanilla by running first: the terrain then covers whatever it occludes,
// so the distances below are vanilla's own and nothing has to be scaled up to clear the world.

#import bevy_render::view::View

const PI: f32 = 3.14159265359;

/// The state of the sky for one frame. The terrain reads the first three fields as its light; the
/// rest only matter here.
struct Sky {
    sky_light: vec4<f32>,
    block_light: vec4<f32>,
    ambient: vec4<f32>,
    /// `rgb` the colour of the sky disc, `a` set when the camera is below the horizon and the dark
    /// disc under it has to be drawn.
    disc: vec4<f32>,
    /// The twilight band: `rgb` its colour, `a` how far it has come in. Zero for most of the day.
    sunrise: vec4<f32>,
    /// Sun, moon and star angles in radians, and how bright the stars are.
    angles: vec4<f32>,
    /// Which layer of the celestial array the moon shows, and how much rain dims the two discs.
    moon: vec4<f32>,
    /// `rgb` the haze the world fades into, which is also what the frame is cleared to, and `a`
    /// how far from the camera the sky has faded entirely into it.
    fog: vec4<f32>,
};

@group(0) @binding(0) var<uniform> view: View;
@group(0) @binding(3) var<uniform> sky: Sky;

// Layer zero is the sun and the eight above it are the moon's phases.
@group(1) @binding(0) var celestials: texture_2d_array<f32>;
@group(1) @binding(1) var celestial_sampler: sampler;

const SKY_DISC_RADIUS: f32 = 512.0;
const SKY_DISC_Y: f32 = 16.0;
/// The dark disc is built at -16 and then translated twelve blocks up.
const DARK_DISC_Y: f32 = -4.0;
/// Both discs are one fan of eight triangles, so twenty-four vertices each.
const DISC_VERTICES: u32 = 24u;

const SUNRISE_STEPS: u32 = 16u;
const SUNRISE_RADIUS: f32 = 120.0;
/// How far the far edge of the band leans away from the horizon.
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
    /// How far the vertex is from the camera, which is all the sky's fog is a function of.
    @location(3) distance: f32,
};

/// The sky is centred on the camera, so a position here is an offset from wherever it stands.
fn to_clip(local: vec3<f32>) -> vec4<f32> {
    return view.clip_from_world * vec4<f32>(view.world_position.xyz + local, 1.0);
}

/// A vertex that must not draw. Every corner of its triangle lands on the same clip position, so
/// the triangle has no area and nothing is rasterised.
fn nothing() -> vec4<f32> {
    return vec4<f32>(0.0, 0.0, 0.0, 1.0);
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

/// The frame the sun, the moon and the stars all hang in: a quarter turn about the vertical, then
/// the swing of the day about what is now the east-west axis.
fn celestial_frame(local: vec3<f32>, angle: f32) -> vec3<f32> {
    return rotate_y(rotate_x(local, angle), -PI / 2.0);
}

/// One corner of the fan the sky disc is built from. Triangle `t` is the centre with ring points
/// `t` and `t + 1`, which is a triangle list saying what vanilla's triangle fan says.
fn disc_corner(index: u32, y: f32) -> vec3<f32> {
    let corner = index % 3u;
    if corner == 0u {
        return vec3<f32>(0.0, y, 0.0);
    }
    // The ring runs the half circle both ways in steps of forty-five degrees, and the radius takes
    // the sign of the height so that both discs face the camera the same way round.
    let step = index / 3u + corner - 1u;
    let angle = (-180.0 + f32(step) * 45.0) * PI / 180.0;
    return vec3<f32>(sign(y) * SKY_DISC_RADIUS * cos(angle), y, SKY_DISC_RADIUS * sin(angle));
}

/// The sky disc, and under it the dark one that stands in for the void when the camera has dropped
/// below the horizon.
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
        out.clip_position = nothing();
    }
    out.distance = length(local);
    return out;
}

/// The band of colour that runs along the horizon at dawn and at dusk: a fan standing on edge,
/// centred on the sun's own side of the sky, opaque at the middle and clear at the rim.
@vertex
fn vertex_sunrise(@builtin(vertex_index) index: u32) -> SkyVertex {
    var out: SkyVertex;
    out.uv = vec2<f32>(0.0);
    out.layer = 0u;
    out.distance = 0.0;
    let alpha = sky.sunrise.a;
    if alpha <= 0.001 {
        out.clip_position = nothing();
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
    // The band thins as it fades, then stands up on the horizon at whichever end of the world the
    // sun is: east while it is still climbing, west once it has passed overhead.
    local.z *= alpha;
    let side = select(0.0, PI, sin(sky.angles.x) < 0.0);
    out.clip_position = to_clip(rotate_x(rotate_z(local, side + PI / 2.0), PI / 2.0));
    out.color = vec4<f32>(sky.sunrise.rgb, alpha * fade);
    return out;
}

/// Two triangles from the four corners vanilla winds a quad with.
fn quad_corner(vertex: u32) -> u32 {
    switch vertex {
        case 0u, 3u: { return 0u; }
        case 1u: { return 1u; }
        case 2u, 4u: { return 2u; }
        default: { return 3u; }
    }
}

/// The sun and the moon, one quad each, flat to the sky and so always square to the camera.
@vertex
fn vertex_celestial(@builtin(vertex_index) index: u32) -> SkyVertex {
    let corner = quad_corner(index % 6u);
    let sign_x = select(-1.0, 1.0, corner == 1u || corner == 2u);
    let sign_z = select(-1.0, 1.0, corner >= 2u);

    let is_moon = index >= 6u;
    let size = select(SUN_SIZE, MOON_SIZE, is_moon);
    let angle = select(sky.angles.x, sky.angles.y, is_moon);
    let local = vec3<f32>(sign_x * size, CELESTIAL_HEIGHT, sign_z * size);

    var uv = vec2<f32>(sign_x, sign_z) * 0.5 + 0.5;
    // The moon's quad is wound off the opposite corners of its sprite, so it hangs mirrored.
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

/// Hash of an index, in place of the seeded generator vanilla walks once at start-up. The star
/// field it lays out has the same distribution and the same counts; the seed does not carry over,
/// so which star is where differs.
fn hash(value: u32) -> u32 {
    var state = value * 747796405u + 2891336453u;
    state = ((state >> ((state >> 28u) + 4u)) ^ state) * 277803737u;
    return (state >> 22u) ^ state;
}

fn random(index: u32) -> f32 {
    return f32(hash(index)) / 4294967296.0;
}

/// One star of however many the draw asks for — fifteen hundred, the number vanilla tries to
/// place. A point drawn from the cube is kept only
/// when it lands inside the unit sphere and clear of its centre, so the field is even across the
/// sky rather than crowded at the corners; a rejected one collapses to nothing here rather than
/// being left out of a buffer.
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
    out.color = vec4<f32>(sky.angles.w);

    let length_squared = dot(point, point);
    if length_squared <= 0.010000001 || length_squared >= 1.0 {
        out.clip_position = nothing();
        return out;
    }

    let centre = normalize(point) * STAR_DISTANCE;
    // The quad is turned to face the middle of the sky, which is where the camera is, and then
    // spun in its own plane so the field does not read as a grid of squares.
    let facing = -normalize(centre);
    let left = normalize(cross(vec3<f32>(0.0, 1.0, 0.0), facing));
    let up = cross(facing, left);

    let corner = quad_corner(index % 6u);
    let flat = vec2<f32>(
        select(-size, size, corner <= 1u),
        select(-size, size, corner == 1u || corner == 2u),
    );
    let turned = vec2<f32>(
        flat.x * cos(spin) + flat.y * sin(spin),
        -flat.x * sin(spin) + flat.y * cos(spin),
    );
    let local = centre + left * turned.x + up * turned.y;
    out.clip_position = to_clip(celestial_frame(local, sky.angles.z));
    return out;
}

@fragment
fn fragment_flat(in: SkyVertex) -> @location(0) vec4<f32> {
    return in.color;
}

/// The disc runs through the same fog every other vertex in the world does, which is what keeps
/// the horizon from being a hard line: the sky is at its own colour overhead, where the disc is
/// nearest, and has faded into the haze by the time it reaches the rim.
@fragment
fn fragment_disc(in: SkyVertex) -> @location(0) vec4<f32> {
    let fog = clamp(in.distance / sky.fog.a, 0.0, 1.0);
    return vec4<f32>(mix(in.color.rgb, sky.fog.rgb, fog), in.color.a);
}

@fragment
fn fragment_celestial(in: SkyVertex) -> @location(0) vec4<f32> {
    // One mip and a nearest filter, so a thirty-two pixel sprite blown across the sky keeps its
    // edges instead of smearing.
    return textureSampleLevel(celestials, celestial_sampler, in.uv, in.layer, 0.0) * in.color;
}
