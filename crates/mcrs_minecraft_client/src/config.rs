use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

use crate::cave::CaveCull;
use crate::mesh::STREAMS;
use crate::render::{
    Budget, FACE_BYTES, MODEL_BYTES, Occlusion, QUAD_BYTES, Raster, Streams, Uploads, Wireframe,
};
use crate::sky_state::SkyEffects;
use crate::stream;

#[cfg(not(target_family = "wasm"))]
const QUAD_MB_PER_FILE: usize = 192;
#[cfg(not(target_family = "wasm"))]
const MODEL_MB_PER_FILE: usize = 640;
#[cfg(not(target_family = "wasm"))]
const FACE_MB_PER_FILE: usize = 256;
#[cfg(target_family = "wasm")]
const QUAD_MB_PER_FILE: usize = 32;
#[cfg(target_family = "wasm")]
const MODEL_MB_PER_FILE: usize = 208;
#[cfg(target_family = "wasm")]
const FACE_MB_PER_FILE: usize = 40;

const UPLOAD_MB: usize = 4;
/// The browser keeps vanilla's default; the desktop asks for three times vanilla's maximum.
#[cfg(not(target_family = "wasm"))]
const VIEW_DISTANCE: u8 = 96;
#[cfg(target_family = "wasm")]
const VIEW_DISTANCE: u8 = 12;
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
    let Some(spec) = knob("VIEW") else {
        return VIEW_DISTANCE;
    };
    match spec.trim().parse::<u8>() {
        Ok(columns) if (2..=MAX_VIEW_DISTANCE).contains(&columns) => columns,
        _ => reject(
            "VIEW",
            &spec,
            format_args!("expected a render distance from 2 to {MAX_VIEW_DISTANCE} columns"),
        )
        .unwrap_or(VIEW_DISTANCE),
    }
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

pub fn fullscreen() -> bool {
    flag("FULLSCREEN", true)
}

/// `LATENCY=<frames>` is how many swapchain images the window may run ahead by, for checking
/// whether a display's drawable recycling is what the frame is waiting on.
pub fn frame_latency() -> Option<std::num::NonZeroU32> {
    let spec = knob("LATENCY")?;
    match spec.trim().parse().ok().and_then(std::num::NonZeroU32::new) {
        Some(frames) => Some(frames),
        None => reject("LATENCY", &spec, "expected a frame count above zero"),
    }
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
    let spec = knob("GPU_HOT")?;
    match spec.trim().parse::<u32>() {
        Ok(workgroups) if workgroups > 0 => Some(workgroups),
        _ => reject("GPU_HOT", &spec, "expected a workgroup count above zero"),
    }
}

/// `FLY=<speed>` holds forward and sprint down from the first tick at that flying speed, in
/// vanilla's units where 0.05 is the default, so a flight can be repeated exactly.
pub fn scripted_flight() -> Option<f64> {
    let spec = knob("FLY")?;
    match spec.trim().parse::<f64>() {
        Ok(speed) if speed > 0.0 => Some(speed),
        _ => reject(
            "FLY",
            &spec,
            "expected a flying speed above zero, 0.05 is vanilla",
        ),
    }
}

/// `TURN=<seconds>` turns a scripted flight round once, that long after launch, so the way
/// back over columns the server took back can be repeated exactly.
pub fn turn_after() -> Option<f32> {
    let spec = knob("TURN")?;
    match spec.trim().parse::<f32>() {
        Ok(seconds) if seconds > 0.0 => Some(seconds),
        _ => reject("TURN", &spec, "expected seconds above zero"),
    }
}

/// `RESOLUTION=<width>x<height>` opens a window of exactly that many pixels instead of the
/// fullscreen one, so a frame can be priced at a stated pixel count.
pub fn resolution() -> Option<(u32, u32)> {
    let spec = knob("RESOLUTION")?;
    let size = spec
        .split_once('x')
        .and_then(|(w, h)| Some((w.trim().parse().ok()?, h.trim().parse().ok()?)));
    match size {
        Some(size) => Some(size),
        None => reject("RESOLUTION", &spec, "expected <width>x<height> in pixels"),
    }
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

pub fn stats_interval() -> Option<f32> {
    knob("STATS").map(|spec| spec.trim().parse().unwrap_or(1.0))
}

pub fn gputrace_path() -> Option<String> {
    knob("GPUTRACE")
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

/// `LOOK=<yaw>,<pitch>` aims the camera somewhere other than where the save
/// left it, in Minecraft degrees.
pub fn look_override() -> Option<(f32, f32)> {
    let look = knob("LOOK")?;
    let angles = look
        .split_once(',')
        .and_then(|(yaw, pitch)| Some((yaw.trim().parse().ok()?, pitch.trim().parse().ok()?)));
    match angles {
        Some(angles) => Some(angles),
        None => reject("LOOK", &look, "expected <yaw>,<pitch> in degrees"),
    }
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
    let ticks = knob("TIME")?;
    match ticks.trim().parse() {
        Ok(ticks) => Some(ticks),
        Err(error) => reject("TIME", ticks.trim(), error),
    }
}

pub struct TerrainLimits {
    pub arena_scale: usize,
    pub groups: usize,
    pub sections: usize,
    /// The side of the tint window the world wraps into, in blocks. Two resident columns this
    /// far apart would share a square, so it has to exceed the view's width.
    pub tint_span: u32,
}

pub fn terrain(limits: TerrainLimits) -> (Arc<Budget>, Uploads, CaveCull, stream::Loader) {
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

    let uploads = Uploads::default();
    let loader = stream::Loader::new(&budget, uploads.clone());
    (
        budget.clone(),
        uploads,
        CaveCull::new(budget.sections),
        loader,
    )
}
