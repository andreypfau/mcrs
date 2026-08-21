// What the three shaders have to agree with each other, and with Rust, about.
//
// `Params` and `Sky` describe uniforms that every pipeline binds from one `#[repr(C)]` struct
// apiece, so a field inserted on one side and not the other does not fail to compile — it binds and
// draws a wrong frame. Declaring them once is the only thing that makes that impossible. The rest
// is the handful of numbers and the one section-placing rule that more than one shader needs.
//
// Nothing here declares a binding. The shaders do not agree on which bindings of group 0 they use,
// and a module that owned them would force them to.

#define_import_path anvil_region_viewer::layout

/// The per-draw uniform, at group 0 binding 1 in every pipeline. Mirrors `Params` in `render.rs`.
struct Params {
    group_base: u32,
    group_count: u32,
    visible_base: u32,
    args_index: u32,
    // The corner of this draw's render region, in blocks. Coordinates in a quad are relative to
    // their own section, so this is what puts the geometry back where it belongs. Explicit scalars
    // rather than a vec3: a vec3 would align to 16 and silently grow the struct.
    origin_x: i32,
    origin_y: i32,
    origin_z: i32,
    /// Where this region's sections start in the sight-line bitset.
    cave_base: u32,
    wireframe: u32,
    /// How far this stream's geometry may reach outside its own section.
    overhang: f32,
    /// The lowest layer number that names an animation rather than a layer of an array.
    animated_from: u32,
    // Where the biome colour map starts in world blocks and how far it reaches. The map covers the
    // loaded window, which does not start at the origin, so a world position has to be shifted and
    // scaled rather than divided by a constant.
    tint_origin_x: i32,
    tint_origin_z: i32,
    tint_span_x: f32,
    tint_span_z: f32,
    /// Where this draw's render region owns its face attributes. A quad's own base counts from
    /// there, so the two have to be added before the buffer is read.
    face_origin: u32,
}

/// The state of the sky for one frame, at group 0 binding 3. Mirrors `Sky` in `render.rs`.
///
/// Vanilla builds a sixteen by sixteen lightmap texture out of the first three fields once a frame
/// and samples it per vertex; there are few enough terms to evaluate them where the sample would
/// have been, which costs no texture and no upload. The terrain reads only those three; the rest
/// only matter to the sky.
struct Sky {
    /// `rgb` the colour sky light arrives in, `a` how much of it the time of day lets through.
    sky_light: vec4<f32>,
    /// `rgb` the tint of a torch at its dimmest, `a` the factor block light is scaled by.
    block_light: vec4<f32>,
    /// The floor under both, which is what keeps a sealed cave from being pure black.
    ambient: vec4<f32>,
    /// `rgb` the colour of the sky disc, `a` set when the camera is below the horizon and the dark
    /// disc under it has to be drawn.
    disc: vec4<f32>,
    /// The twilight band: `rgb` its colour, `a` how far it has come in. Zero for most of the day.
    sunrise: vec4<f32>,
    /// The sun's angle in radians, and how bright the stars are. The moon stands opposite the sun
    /// and the stars turn with it, so neither carries an angle of its own.
    angles: vec4<f32>,
    /// Which layer of the celestial array the moon shows, and how much rain dims the two discs.
    moon: vec4<f32>,
    /// `rgb` the haze the world fades into, which is also what the frame is cleared to, and `a` how
    /// far from the camera the sky has faded entirely into it.
    fog: vec4<f32>,
    /// The colour the cloud layer is lit and tinted by, `a` included.
    cloud_color: vec4<f32>,
    /// Where the underside of the cloud layer sits, how far the field has drifted in x and z, and
    /// the distance the clouds have faded out by.
    cloud: vec4<f32>,
}

/// Blocks along one edge of a section.
const SECTION_SIZE: f32 = 16.0;

/// What the culling pass leaves in the visible list where a run did not survive, and so the one
/// value the two passes have to read the same way. Only the blended streams hold any: theirs is
/// laid out by each run's own place in the draw so that the order the quads blend in does not
/// change between frames, which means the list has a slot for every quad whether it is drawn or not.
const CULLED: u32 = 0xffffffffu;

// Where a section sits inside its render region. Both passes unpack this out of the same word.
const LOCAL_X_WORD: u32 = 0u;
const LOCAL_X_SHIFT: u32 = 0u;
const LOCAL_X_BITS: u32 = 4u;
const LOCAL_Y_WORD: u32 = 0u;
const LOCAL_Y_SHIFT: u32 = 4u;
const LOCAL_Y_BITS: u32 = 3u;
const LOCAL_Z_WORD: u32 = 0u;
const LOCAL_Z_SHIFT: u32 = 7u;
const LOCAL_Z_BITS: u32 = 4u;

/// Where the section a quad names starts, in world blocks. A coordinate in a quad is relative to
/// its own section, and the region corner arrives with the draw, so this is the whole of what puts
/// the geometry back where it belongs. The region corner is passed in rather than read from
/// `params`, since a module cannot reach the importing shader's bindings.
fn section_origin(section: u32, region: vec3<f32>) -> vec3<f32> {
    let local = vec3<f32>(
        f32(extractBits(section, LOCAL_X_SHIFT, LOCAL_X_BITS)),
        f32(extractBits(section, LOCAL_Y_SHIFT, LOCAL_Y_BITS)),
        f32(extractBits(section, LOCAL_Z_SHIFT, LOCAL_Z_BITS)),
    );
    return region + local * SECTION_SIZE;
}

/// A vertex that must not draw. Every corner of its triangle lands here, so the triangle has no
/// area and nothing is rasterised — which is how a quad the culling pass threw away leaves the
/// stage without anything downstream having to know it was ever here.
fn degenerate() -> vec4<f32> {
    return vec4<f32>(0.0, 0.0, 0.0, 1.0);
}
