use std::num::NonZeroU32;

use mcrs_minecraft_mesh::STREAMS;
use mcrs_minecraft_render::sky::SkyEffects;

use crate::config::{Knobs, reject};

impl Knobs {
    /// `HOT=1` keeps one core spinning for the whole run. The frame sleeps in the swapchain acquire
    /// and the performance cluster clocks down while it does, so the same work then measures two to
    /// three times longer; with the cluster held at speed the numbers are the engine's own.
    pub fn hot_clocks(&self) -> bool {
        self.flag("HOT", false)
    }

    /// `LATENCY=<frames>` is how many swapchain images the window may run ahead by, for checking
    /// whether a display's drawable recycling is what the frame is waiting on.
    pub fn frame_latency(&self) -> Option<NonZeroU32> {
        self.parsed("LATENCY", |_| true, "expected a frame count above zero")
    }

    /// `MONITOR=primary` opens the window on the system's primary display instead of
    /// on the fastest one.
    pub fn monitor_primary(&self) -> bool {
        self.knob("MONITOR").map(str::trim) == Some("primary")
    }

    /// `CENSUS=<seconds>` writes one line per interval tallying every traced column
    /// by lifecycle stage, so a headless run can be timed without reading the window.
    pub fn census_interval(&self) -> Option<f32> {
        self.knob("CENSUS")
            .map(|spec| spec.trim().parse().unwrap_or(1.0))
    }

    /// `TRACY_PREVIEW=1` streams frame images to the profiler, which costs a readback per frame and
    /// so stays off in a capture meant for timing.
    pub fn tracy_preview(&self) -> bool {
        self.flag("TRACY_PREVIEW", false)
    }

    /// `CHUNK_GUARD=1` takes the client down the moment the player stands in a
    /// column it does not hold, `=warn` only says so, and `0` or unset is off.
    ///
    /// A break that self-heals a frame later cannot be missed by a log line, which
    /// is why the loud form is the default once the knob is set at all.
    pub fn chunk_guard(&self) -> Option<Guard> {
        guard(self.knob("CHUNK_GUARD"))
    }

    /// `LIGHT_GUARD` reads the same way; `warn` keeps the process alive so a whole
    /// load can be measured.
    pub fn light_guard(&self) -> Option<Guard> {
        guard(self.knob("LIGHT_GUARD"))
    }

    pub fn gputrace_path(&self) -> Option<String> {
        self.knob("GPUTRACE").map(str::to_owned)
    }

    pub fn raster_fraction(&self) -> Option<f32> {
        let spec = self.knob("RASTER")?;
        match spec.trim().parse::<f32>() {
            Ok(fraction) if (0.0..=1.0).contains(&fraction) && fraction > 0.0 => Some(fraction),
            _ => {
                eprintln!("MCRS_RASTER takes a fraction between 0 and 1");
                None
            }
        }
    }

    pub fn draw_mask(&self) -> Option<u32> {
        let spec = self.knob("DRAWS")?;
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
    pub fn sky_draws_only(&self) -> Option<SkyEffects> {
        let list = self.knob("SKY")?;
        match SkyEffects::parse(list) {
            Ok(effects) => Some(effects),
            Err(error) => reject("SKY", list, error),
        }
    }

    /// `FLY=<speed>` holds forward and sprint down from the first tick at that flying speed, in
    /// blocks per tick, so a flight can be repeated exactly.
    pub fn scripted_flight(&self) -> Option<f64> {
        self.parsed(
            "FLY",
            |&speed| speed > 0.0,
            "expected a flying speed above zero, in blocks per tick",
        )
    }

    /// `TURN=<seconds>` turns a scripted flight round once, that long after launch, so the way
    /// back over columns the server took back can be repeated exactly.
    pub fn turn_after(&self) -> Option<f32> {
        self.parsed(
            "TURN",
            |&seconds| seconds > 0.0,
            "expected seconds above zero",
        )
    }
}

fn guard(value: Option<&str>) -> Option<Guard> {
    match value.map(str::trim) {
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
