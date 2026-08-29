use crate::mesh::STREAMS;
use crate::render::{Raster, Streams, Wireframe};

const QUAD_MB_PER_FILE: usize = 32;
const MODEL_MB_PER_FILE: usize = 208;
const FACE_MB_PER_FILE: usize = 40;

const UPLOAD_MB: usize = 4;

const REGION_WINDOW: usize = 2;

fn numbers<T: std::str::FromStr>(spec: &str) -> Vec<T> {
    spec.split(',')
        .filter_map(|n| n.trim().parse().ok())
        .collect()
}

pub fn upload_budget() -> usize {
    std::env::var("ANVIL_UPLOAD")
        .ok()
        .and_then(|megabytes| megabytes.parse::<usize>().ok())
        .unwrap_or(UPLOAD_MB)
        << 20
}

pub fn arena_budget() -> (usize, usize, usize) {
    let default = (QUAD_MB_PER_FILE, MODEL_MB_PER_FILE, FACE_MB_PER_FILE);
    let Ok(spec) = std::env::var("ANVIL_ARENA") else {
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

pub fn drawn_streams() -> Streams {
    let Ok(spec) = std::env::var("ANVIL_STREAMS") else {
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
    let Ok(spec) = std::env::var("ANVIL_RASTER") else {
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
    let spec = std::env::var("ANVIL_CENTER").ok()?;
    match numbers::<i32>(&spec)[..] {
        [x, z] => Some([x, z]),
        _ => {
            eprintln!("ANVIL_CENTER needs two region coordinates: x,z");
            None
        }
    }
}

pub fn region_window() -> usize {
    std::env::var("ANVIL_WINDOW")
        .ok()
        .and_then(|size| size.parse().ok())
        .unwrap_or(REGION_WINDOW)
        .max(1)
}

pub fn gputrace_path() -> Option<String> {
    std::env::var("ANVIL_GPUTRACE").ok()
}

pub fn wireframe() -> Wireframe {
    Wireframe(std::env::var("ANVIL_WIREFRAME").is_ok_and(|on| on != "0"))
}
