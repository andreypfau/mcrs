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

const REGION_WINDOW: usize = 2;

static KNOBS: OnceLock<HashMap<String, String>> = OnceLock::new();

/// Names a knob without its `ANVIL_` prefix, so a source that is not the
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
    std::env::var(format!("ANVIL_{name}")).ok()
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
            eprintln!("ANVIL_ARENA needs three sizes in megabytes: quads,models,faces");
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

pub fn drawn_streams() -> Streams {
    let Some(spec) = knob("STREAMS") else {
        return Streams::default();
    };
    let mut mask = 0;
    for name in spec.split(',') {
        match name.trim().parse::<u32>() {
            Ok(stream) if (stream as usize) < STREAMS => mask |= 1 << stream,
            _ => eprintln!("ANVIL_STREAMS takes stream numbers 0..{}", STREAMS - 1),
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
            eprintln!("ANVIL_RASTER takes a fraction between 0 and 1");
            Raster::default()
        }
    }
}

pub fn window_centre() -> Option<[i32; 2]> {
    let spec = knob("CENTER")?;
    match numbers::<i32>(&spec)[..] {
        [x, z] => Some([x, z]),
        _ => {
            eprintln!("ANVIL_CENTER needs two region coordinates: x,z");
            None
        }
    }
}

pub fn region_window() -> usize {
    knob("WINDOW")
        .and_then(|size| size.parse().ok())
        .unwrap_or(REGION_WINDOW)
        .max(1)
}

pub fn gputrace_path() -> Option<String> {
    knob("GPUTRACE")
}

pub fn wireframe() -> Wireframe {
    Wireframe(knob("WIREFRAME").is_some_and(|on| on != "0"))
}
