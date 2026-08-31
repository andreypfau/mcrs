use std::collections::HashMap;
use std::sync::OnceLock;

use crate::mesh::STREAMS;
use crate::render::{Raster, Streams, Wireframe};

const QUAD_MB_PER_FILE: usize = 32;
const MODEL_MB_PER_FILE: usize = 208;
const FACE_MB_PER_FILE: usize = 40;

const UPLOAD_MB: usize = 4;

/// A render distance of 16 holds around two and a half million quads, and the visible list only
/// ever holds what a frame draws, so this leaves room to spare and is given back between frames.
const VISIBLE_MB: usize = 32;

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

pub fn visible_budget() -> usize {
    knob("VISIBLE")
        .and_then(|megabytes| megabytes.parse::<usize>().ok())
        .unwrap_or(VISIBLE_MB)
        .max(1)
        * 1_000_000
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
pub fn stats_interval() -> Option<f32> {
    knob("STATS").map(|spec| spec.trim().parse().unwrap_or(1.0))
}

pub fn gputrace_path() -> Option<String> {
    knob("GPUTRACE")
}

pub fn wireframe() -> Wireframe {
    Wireframe(knob("WIREFRAME").is_some_and(|on| on != "0"))
}
