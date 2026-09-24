use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

use crate::cave::CaveCull;
use crate::render::{
    Budget, FACE_BYTES, MODEL_BYTES, Occlusion, QUAD_BYTES, Raster, Streams, Uploads, Wireframe,
};
use crate::sky_state::SkyEffects;
use mcrs_minecraft_mesh::STREAMS;

#[cfg(not(target_family = "wasm"))]
const QUAD_MB_PER_FILE: usize = 192;
#[cfg(not(target_family = "wasm"))]
const MODEL_MB_PER_FILE: usize = 640;
#[cfg(not(target_family = "wasm"))]
const FACE_MB_PER_FILE: usize = 512;
#[cfg(target_family = "wasm")]
const QUAD_MB_PER_FILE: usize = 32;
#[cfg(target_family = "wasm")]
const MODEL_MB_PER_FILE: usize = 208;
#[cfg(target_family = "wasm")]
const FACE_MB_PER_FILE: usize = 80;

const UPLOAD_MB: usize = 4;

/// Bevy's stock split caps async compute at four threads whatever the machine has, and on
/// this one the mesher, the column decode and the embedded server's lighting all live there.
/// Everything but a handful of cores kept for the render and io pools does better.
#[cfg(not(target_family = "wasm"))]
fn async_compute_threads() -> usize {
    bevy::tasks::available_parallelism()
        .saturating_sub(4)
        .max(4)
}

/// The browser has no worker pool to spread across.
#[cfg(target_family = "wasm")]
fn async_compute_threads() -> usize {
    1
}

/// Sections the mesher may admit in one frame, and how many may be in flight across frames.
/// The per-frame figure paces the main-thread placement that follows each finished mesh; the
/// in-flight figure is what keeps the pool fed while a frame is long. On the web both jobs run
/// on the thread that renders, so the frame is the budget and vanilla's pacing is right.
#[cfg(not(target_family = "wasm"))]
const MESH_IN_FLIGHT: usize = 2048;
#[cfg(not(target_family = "wasm"))]
const MESH_PER_FRAME: usize = 2048;
#[cfg(target_family = "wasm")]
const MESH_IN_FLIGHT: usize = 128;
#[cfg(target_family = "wasm")]
const MESH_PER_FRAME: usize = 32;

const VIEW_DISTANCE: u8 = 96;
pub const MAX_VIEW_DISTANCE: u8 = 96;

static KNOBS: OnceLock<HashMap<String, String>> = OnceLock::new();

/// Names a knob without its `MCRS_` prefix, so a source that is not the
/// environment — the browser has none, and reads the query string instead —
/// can supply the same values. Only the first call is kept, and it has to come
/// before the first read.
pub fn seed(knobs: HashMap<String, String>) {
    let _ = KNOBS.set(knobs);
}

fn knob(name: &str) -> Option<String> {
    match KNOBS.get() {
        Some(knobs) => knobs.get(name).cloned(),
        None => from_environment(name),
    }
}

#[cfg(not(target_family = "wasm"))]
fn from_environment(name: &str) -> Option<String> {
    std::env::var(format!("MCRS_{name}")).ok()
}

#[cfg(target_family = "wasm")]
fn from_environment(name: &str) -> Option<String> {
    crate::web::query(&name.to_ascii_lowercase())
}

fn numbers<T: std::str::FromStr>(spec: &str) -> Vec<T> {
    spec.split(',')
        .filter_map(|n| n.trim().parse().ok())
        .collect()
}

/// `VIEW=<columns>` is the render distance the client asks the server for.
pub fn view_distance() -> u8 {
    parsed(
        "VIEW",
        |columns| (2..=MAX_VIEW_DISTANCE).contains(columns),
        format_args!("expected a render distance from 2 to {MAX_VIEW_DISTANCE} columns"),
    )
    .unwrap_or(VIEW_DISTANCE)
}

pub fn upload_budget() -> usize {
    knob("UPLOAD")
        .and_then(|megabytes| megabytes.parse::<usize>().ok())
        .unwrap_or(UPLOAD_MB)
        << 20
}

pub fn arena_budget() -> (usize, usize, usize) {
    let default = (QUAD_MB_PER_FILE, MODEL_MB_PER_FILE, FACE_MB_PER_FILE);
    let Some(spec) = knob("ARENA") else {
        return default;
    };
    match numbers::<usize>(&spec)[..] {
        [quads, models, faces] => (quads.max(1), models.max(1), faces.max(1)),
        _ => {
            eprintln!("MCRS_ARENA needs three sizes in megabytes: quads,models,faces");
            default
        }
    }
}

fn flag(name: &str, default: bool) -> bool {
    match knob(name).as_deref().map(str::trim) {
        Some("0" | "false" | "off" | "no") => false,
        Some("1" | "true" | "on" | "yes") => true,
        Some(other) => {
            eprintln!("MCRS_{name}={other} takes 0 or 1");
            default
        }
        None => default,
    }
}

/// `FULLSCREEN=1` takes over a whole display; a plain window is the default, so
/// a run never seizes the screen the work is being done on.
pub fn fullscreen() -> bool {
    flag("FULLSCREEN", false)
}

/// `LATENCY=<frames>` is how many swapchain images the window may run ahead by, for checking
/// whether a display's drawable recycling is what the frame is waiting on.
pub fn frame_latency() -> Option<std::num::NonZeroU32> {
    parsed("LATENCY", |_| true, "expected a frame count above zero")
}

/// `TRACY_PREVIEW=1` streams frame images to the profiler, which costs a readback per frame and
/// so stays off in a capture meant for timing.
pub fn tracy_preview() -> bool {
    flag("TRACY_PREVIEW", false)
}

/// `HOT=1` keeps one core spinning for the whole run. The frame sleeps in the swapchain acquire
/// and the performance cluster clocks down while it does, so the same work then measures two to
/// three times longer; with the cluster held at speed the numbers are the engine's own.
pub fn hot_clocks() -> bool {
    flag("HOT", false)
}

/// `GPU_HOT=<workgroups>` burns that many workgroups of arithmetic after the frame's own passes.
/// A composited window presents at the display's rate and leaves the GPU idle most of the frame,
/// and it clocks down while it waits: the same cull dispatch read 0.25 ms in a 60 Hz window and
/// 0.11 ms fullscreen on a 120 Hz display, and the governor only lets go of the deadline once
/// the GPU is saturated: 16384 workgroups did that on an M4 Max at 60 Hz, where the same cull
/// read 0.058 ms. With the GPU held busy the pass timestamps are the code's own.
pub fn gpu_hot() -> Option<u32> {
    parsed(
        "GPU_HOT",
        |&workgroups| workgroups > 0,
        "expected a workgroup count above zero",
    )
}

/// `FLY=<speed>` holds forward and sprint down from the first tick at that flying speed, in
/// vanilla's units where 0.05 is the default, so a flight can be repeated exactly.
pub fn scripted_flight() -> Option<f64> {
    parsed(
        "FLY",
        |&speed| speed > 0.0,
        "expected a flying speed above zero, 0.05 is vanilla",
    )
}

/// `TURN=<seconds>` turns a scripted flight round once, that long after launch, so the way
/// back over columns the server took back can be repeated exactly.
pub fn turn_after() -> Option<f32> {
    parsed(
        "TURN",
        |&seconds| seconds > 0.0,
        "expected seconds above zero",
    )
}

/// `RESOLUTION=<width>x<height>` opens a window of exactly that many pixels instead of the
/// fullscreen one, so a frame can be priced at a stated pixel count.
pub fn resolution() -> Option<(u32, u32)> {
    pair("RESOLUTION", 'x', "expected <width>x<height> in pixels")
}

/// Off by default so a frame time is readable: with vsync the frame reports the
/// refresh interval no matter what the renderer costs.
pub fn vsync() -> bool {
    flag("VSYNC", false)
}

pub fn drawn_streams() -> Streams {
    let Some(spec) = knob("STREAMS") else {
        return Streams::default();
    };
    let mut mask = 0;
    for name in spec.split(',') {
        match name.trim().parse::<u32>() {
            Ok(stream) if (stream as usize) < STREAMS => mask |= 1 << stream,
            _ => eprintln!("MCRS_STREAMS takes stream numbers 0..{}", STREAMS - 1),
        }
    }
    Streams(mask)
}

pub fn raster_fraction() -> Raster {
    let Some(spec) = knob("RASTER") else {
        return Raster::default();
    };
    match spec.trim().parse::<f32>() {
        Ok(fraction) if (0.0..=1.0).contains(&fraction) && fraction > 0.0 => Raster(fraction),
        _ => {
            eprintln!("MCRS_RASTER takes a fraction between 0 and 1");
            Raster::default()
        }
    }
}

/// Seconds between debug-screen lines written to the log, for a run whose
/// window cannot be read.
pub fn chunk_map() -> bool {
    flag("CHUNK_MAP", false)
}

/// `CENSUS=<seconds>` writes one line per interval tallying every traced column
/// by lifecycle stage, so a headless run can be timed without reading the window.
pub fn census_interval() -> Option<f32> {
    knob("CENSUS").map(|spec| spec.trim().parse().unwrap_or(1.0))
}

/// `MONITOR=primary` opens the window on the system's primary display instead of
/// on the fastest one.
pub fn monitor_primary() -> bool {
    knob("MONITOR").as_deref().map(str::trim) == Some("primary")
}

pub fn light_levels() -> bool {
    flag("LIGHT_LEVELS", false)
}

/// `CHUNK_GUARD=1` takes the client down the moment the player stands in a
/// column it does not hold, `=warn` only says so, and `0` or unset is off.
///
/// A break that self-heals a frame later cannot be missed by a log line, which
/// is why the loud form is the default once the knob is set at all.
pub fn chunk_guard() -> Option<Guard> {
    guard(knob("CHUNK_GUARD"))
}

/// `LIGHT_GUARD` reads the same way; `warn` keeps the process alive so a whole
/// load can be measured.
pub fn light_guard() -> Option<Guard> {
    guard(knob("LIGHT_GUARD"))
}

fn guard(value: Option<String>) -> Option<Guard> {
    match value.as_deref().map(str::trim) {
        None | Some("0") => None,
        Some("warn") => Some(Guard::Warn),
        Some(_) => Some(Guard::Panic),
    }
}

#[derive(Copy, Clone, PartialEq, Eq)]
pub enum Guard {
    Warn,
    Panic,
}

/// Bevy's stock split gives async compute a quarter of the cores capped at four, which on a
/// sixteen-core machine leaves meshing, column decode and the embedded server's lighting to
/// share four threads while twelve idle. `ASYNC=<threads>` moves the cap.
pub fn task_pool_options() -> bevy::app::TaskPoolOptions {
    let mut options = bevy::app::TaskPoolOptions::default();
    options.async_compute.max_threads = knob("ASYNC")
        .and_then(|spec| spec.trim().parse().ok())
        .unwrap_or_else(async_compute_threads);
    options.async_compute.percent = 1.0;
    options
}

/// `MESH=<per frame>` and `MESH_FLIGHT=<jobs>` bound how fast the mesher may run: how many
/// sections it admits in one frame and how many may be in flight at once.
pub fn mesh_per_frame() -> usize {
    knob("MESH")
        .and_then(|spec| spec.trim().parse().ok())
        .unwrap_or(MESH_PER_FRAME)
}

pub fn mesh_in_flight() -> usize {
    knob("MESH_FLIGHT")
        .and_then(|spec| spec.trim().parse().ok())
        .unwrap_or(MESH_IN_FLIGHT)
}

pub fn stats_interval() -> Option<f32> {
    knob("STATS").map(|spec| spec.trim().parse().unwrap_or(1.0))
}

pub fn gputrace_path() -> Option<String> {
    knob("GPUTRACE")
}

/// Vanilla's "Smooth Lighting": ambient occlusion and light blended across each face. Off draws
/// every face at the flat light in front of it.
pub fn smooth_lighting() -> bool {
    flag("SMOOTH_LIGHTING", true)
}

/// Vanilla's "Brightness" slider, from 0 (Moody) through 0.5 (the default) to 1 (Bright).
pub fn brightness() -> f32 {
    parsed(
        "BRIGHTNESS",
        |value: &f32| (0.0..=1.0).contains(value),
        "a brightness from 0 to 1",
    )
    .unwrap_or(0.5)
}

pub fn wireframe() -> Wireframe {
    Wireframe(knob("WIREFRAME").is_some_and(|on| on != "0"))
}

/// `OCCLUSION=0` draws every group the frustum and the cave graph keep, without the depth
/// pyramid test and its second pass.
pub fn occlusion() -> Occlusion {
    Occlusion(flag("OCCLUSION", true))
}

/// The knob's spelling in the message a bad value produces, which is the
/// spelling whoever set it typed.
#[cfg(not(target_family = "wasm"))]
fn spelled(name: &str) -> String {
    format!("MCRS_{name}")
}

#[cfg(target_family = "wasm")]
fn spelled(name: &str) -> String {
    format!("?{}", name.to_ascii_lowercase())
}

/// A knob set to something unusable is a typo in a run that was asked for, so
/// the native binary refuses to start. The browser has no exit status to carry
/// that, and drops the knob after saying so.
#[cfg(not(target_family = "wasm"))]
fn reject<T>(name: &str, value: &str, expected: impl std::fmt::Display) -> Option<T> {
    eprintln!("{}={value}: {expected}", spelled(name));
    std::process::exit(1);
}

#[cfg(target_family = "wasm")]
fn reject<T>(name: &str, value: &str, expected: impl std::fmt::Display) -> Option<T> {
    bevy::log::error!("{}={value}: {expected}", spelled(name));
    None
}

fn parsed<T: std::str::FromStr>(
    name: &str,
    check: impl FnOnce(&T) -> bool,
    expected: impl std::fmt::Display,
) -> Option<T> {
    let spec = knob(name)?;
    match spec.trim().parse() {
        Ok(value) if check(&value) => Some(value),
        _ => reject(name, &spec, expected),
    }
}

fn pair<T: std::str::FromStr>(name: &str, separator: char, expected: &str) -> Option<(T, T)> {
    let spec = knob(name)?;
    spec.split_once(separator)
        .and_then(|(a, b)| Some((a.trim().parse().ok()?, b.trim().parse().ok()?)))
        .or_else(|| reject(name, &spec, expected))
}

/// `LOOK=<yaw>,<pitch>` aims the camera somewhere other than where the save
/// left it, in Minecraft degrees.
pub fn look_override() -> Option<(f32, f32)> {
    pair("LOOK", ',', "expected <yaw>,<pitch> in degrees")
}

/// `SKY=disc,twilight,celestial,stars,clouds` draws only the passes it lists,
/// which is how a frame gets priced one pass at a time.
pub fn sky_draws_only() -> Option<SkyEffects> {
    let list = knob("SKY")?;
    match SkyEffects::parse(&list) {
        Ok(effects) => Some(effects),
        Err(error) => reject("SKY", &list, error),
    }
}

/// `TIME=<ticks>` pins every clock and stops them, so a scripted screenshot
/// lands on the tick it asked for.
pub fn frozen_time() -> Option<i64> {
    parsed("TIME", |_| true, "expected a tick count")
}

/// `GUI_SCALE=<n>` pins the GUI scale; `0` or unset picks the largest scale
/// that keeps 320x240 GUI units on screen, as vanilla's auto setting does.
pub fn gui_scale() -> u32 {
    parsed("GUI_SCALE", |_| true, "expected a whole GUI scale").unwrap_or(0)
}

/// `SCREEN=inventory` opens that screen at start, so a capture is deterministic.
pub fn initial_screen() -> crate::inventory::Screen {
    use crate::inventory::Screen;
    match knob("SCREEN").as_deref() {
        None | Some("none") => Screen::None,
        Some("inventory") => Screen::Inventory,
        Some(other) => {
            reject("SCREEN", other, "expected none or inventory").unwrap_or(Screen::None)
        }
    }
}

/// `CURSOR=<x>,<y>` pins the cursor in GUI units for the screens, in place of the pointer.
pub fn gui_cursor() -> Option<bevy::math::IVec2> {
    pair("CURSOR", ',', "expected <x>,<y> in GUI units").map(|(x, y)| bevy::math::IVec2::new(x, y))
}

#[derive(Clone, Copy)]
pub struct TerrainLimits {
    pub arena_scale: usize,
    pub groups: usize,
    pub sections: usize,
    /// The side of the tint window the world wraps into, in blocks. Two resident columns this
    /// far apart would share a square, so it has to exceed the view's width.
    pub tint_span: u32,
}

pub fn terrain(limits: TerrainLimits) -> (Arc<Budget>, Uploads, CaveCull) {
    let (quad_mb, model_mb, face_mb) = arena_budget();
    let budget = Arc::new(Budget {
        quads: quad_mb * limits.arena_scale * 1_000_000 / QUAD_BYTES,
        models: model_mb * limits.arena_scale * 1_000_000 / MODEL_BYTES,
        faces: face_mb * limits.arena_scale * 1_000_000 / FACE_BYTES,
        groups: limits.groups,
        sections: limits.sections,
        tint_size: [limits.tint_span; 2],
    });

    bevy::log::info!(
        quad_mb = (budget.quads * QUAD_BYTES) / 1_000_000,
        model_mb = (budget.models * MODEL_BYTES) / 1_000_000,
        face_mb = (budget.faces * FACE_BYTES) / 1_000_000,
        "meshing the columns the server sends"
    );

    let cave = CaveCull::new(budget.sections);
    (budget, Uploads::default(), cave)
}
