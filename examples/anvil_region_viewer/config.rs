use bevy::prelude::*;
use bevy::window::{MonitorSelection, VideoModeSelection, WindowMode};

use crate::camera::{Orbit, Sweep};
use crate::mesh::STREAMS;
use crate::render::{Raster, Streams};

const QUAD_MB_PER_FILE: usize = 32;
const MODEL_MB_PER_FILE: usize = 208;
const FACE_MB_PER_FILE: usize = 40;

const UPLOAD_MB: usize = 4;

fn numbers<T: std::str::FromStr>(spec: &str) -> Vec<T> {
    spec.split(',').filter_map(|n| n.trim().parse().ok()).collect()
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

pub fn window_centre() -> [i32; 2] {
    let Ok(spec) = std::env::var("ANVIL_CENTER") else {
        return [0, 0];
    };
    match numbers::<i32>(&spec)[..] {
        [x, z] => [x, z],
        _ => {
            eprintln!("ANVIL_CENTER needs two region coordinates: x,z");
            [0, 0]
        }
    }
}

pub fn sweep() -> Sweep {
    Sweep(
        std::env::var("ANVIL_SWEEP")
            .ok()
            .and_then(|speed| speed.parse().ok())
            .unwrap_or(0.0),
    )
}

pub fn starting_orbit() -> Orbit {
    let default = Orbit {
        yaw: 0.8,
        pitch: 0.6,
        radius: 420.0,
        target: Vec3::new(256.0, 64.0, 256.0),
    };
    let Ok(spec) = std::env::var("ANVIL_VIEW") else {
        return default;
    };
    let [yaw, pitch, radius, x, y, z] = numbers::<f32>(&spec)[..] else {
        warn!("ANVIL_VIEW needs six numbers: yaw,pitch,radius,x,y,z");
        return default;
    };
    Orbit {
        yaw,
        pitch,
        radius,
        target: Vec3::new(x, y, z),
    }
}

pub fn overlay_shown() -> bool {
    !std::env::var("ANVIL_OVERLAY").is_ok_and(|on| on == "0")
}

pub fn screenshot_path() -> Option<String> {
    std::env::var("ANVIL_SCREENSHOT").ok()
}

pub fn monitor_spec() -> Option<String> {
    std::env::var("ANVIL_MONITOR").ok()
}

pub fn fullscreen_mode(monitor: MonitorSelection) -> Option<WindowMode> {
    match std::env::var("ANVIL_FULLSCREEN").as_deref() {
        Ok("exclusive") => Some(WindowMode::Fullscreen(monitor, VideoModeSelection::Current)),
        Ok(_) => Some(WindowMode::BorderlessFullscreen(monitor)),
        Err(_) => None,
    }
}
