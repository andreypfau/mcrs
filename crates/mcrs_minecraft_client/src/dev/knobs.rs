use std::num::NonZeroU32;

use mcrs_minecraft_mesh::STREAMS;
use mcrs_minecraft_render::sky::SkyEffects;

use crate::config::{flag, knob, parsed, reject};

/// `HOT=1` keeps one core spinning for the whole run. The frame sleeps in the swapchain acquire
/// and the performance cluster clocks down while it does, so the same work then measures two to
/// three times longer; with the cluster held at speed the numbers are the engine's own.
pub fn hot_clocks() -> bool {
    flag("HOT", false)
}

/// `LATENCY=<frames>` is how many swapchain images the window may run ahead by, for checking
/// whether a display's drawable recycling is what the frame is waiting on.
pub fn frame_latency() -> Option<NonZeroU32> {
    parsed("LATENCY", |_| true, "expected a frame count above zero")
}

/// `MONITOR=primary` opens the window on the system's primary display instead of
/// on the fastest one.
pub fn monitor_primary() -> bool {
    knob("MONITOR").as_deref().map(str::trim) == Some("primary")
}

/// `CENSUS=<seconds>` writes one line per interval tallying every traced column
/// by lifecycle stage, so a headless run can be timed without reading the window.
pub fn census_interval() -> Option<f32> {
    knob("CENSUS").map(|spec| spec.trim().parse().unwrap_or(1.0))
}

/// `TRACY_PREVIEW=1` streams frame images to the profiler, which costs a readback per frame and
/// so stays off in a capture meant for timing.
pub fn tracy_preview() -> bool {
    flag("TRACY_PREVIEW", false)
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

pub fn gputrace_path() -> Option<String> {
    knob("GPUTRACE")
}

pub fn raster_fraction() -> Option<f32> {
    let spec = knob("RASTER")?;
    match spec.trim().parse::<f32>() {
        Ok(fraction) if (0.0..=1.0).contains(&fraction) && fraction > 0.0 => Some(fraction),
        _ => {
            eprintln!("MCRS_RASTER takes a fraction between 0 and 1");
            None
        }
    }
}

pub fn draw_mask() -> Option<u32> {
    let spec = knob("DRAWS")?;
    let mut mask = 0;
    for name in spec.split(',') {
        match name.trim().parse::<u32>() {
            Ok(draw) if (draw as usize) < STREAMS => mask |= 1 << draw,
            _ => eprintln!("MCRS_DRAWS takes draw numbers 0..{}", STREAMS - 1),
        }
    }
    Some(mask)
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
