use std::collections::HashMap;
use std::sync::Arc;

#[cfg(all(feature = "singleplayer", not(target_family = "wasm")))]
use std::net::{Ipv4Addr, SocketAddr};

use crate::cave::CaveCull;
use bevy::app::App;
use bevy::ecs::resource::Resource;
use mcrs_minecraft_render::{Budget, FACE_BYTES, MODEL_BYTES, QUAD_BYTES, Uploads};

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
/// Half the cores go there; the compute pool takes what io leaves, since it runs every parallel
/// system of the main and render worlds and one thread there serialises the whole frame.
#[cfg(not(target_family = "wasm"))]
fn async_compute_threads() -> usize {
    (bevy::tasks::available_parallelism() / 2).max(4)
}

#[cfg(not(target_family = "wasm"))]
pub const IO_THREADS: usize = 2;
#[cfg(target_family = "wasm")]
pub const IO_THREADS: usize = 1;

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

/// The `MCRS_` settings of a run, named without the prefix; with none set, every setting takes
/// its default. Where a name repeats, the first value counts.
#[derive(Resource, Clone, Debug, Default)]
pub struct Knobs {
    values: Arc<HashMap<String, String>>,
    overlay_times_encoders: bool,
}

impl Knobs {
    /// `overlay_times_encoders` is set while the system's profiling overlay times every encoder.
    pub fn new(
        values: impl IntoIterator<Item = (String, String)>,
        overlay_times_encoders: bool,
    ) -> Self {
        let mut map = HashMap::new();
        for (name, value) in values {
            map.entry(name).or_insert(value);
        }
        Self {
            values: Arc::new(map),
            overlay_times_encoders,
        }
    }

    pub(crate) fn of(app: &App) -> Self {
        app.world()
            .get_resource::<Self>()
            .expect("the app holds its Knobs before the client plugins are added")
            .clone()
    }

    pub(crate) fn knob(&self, name: &str) -> Option<&str> {
        self.values.get(name).map(String::as_str)
    }

    /// `VIEW=<columns>` is the render distance the client asks the server for.
    pub fn view_distance(&self) -> u8 {
        self.parsed(
            "VIEW",
            |columns| (2..=MAX_VIEW_DISTANCE).contains(columns),
            format_args!("expected a render distance from 2 to {MAX_VIEW_DISTANCE} columns"),
        )
        .unwrap_or(VIEW_DISTANCE)
    }

    pub fn username(&self) -> String {
        self.knob("USERNAME").unwrap_or("Player").to_owned()
    }

    pub fn server(&self) -> Option<&str> {
        self.knob("SERVER")
    }

    #[cfg(target_family = "wasm")]
    pub fn server_certificate(&self) -> Option<&str> {
        self.knob("CERT")
    }

    pub fn upload_budget(&self) -> usize {
        self.knob("UPLOAD")
            .and_then(|megabytes| megabytes.parse::<usize>().ok())
            .unwrap_or(UPLOAD_MB)
            << 20
    }

    pub fn arena_budget(&self) -> (usize, usize, usize) {
        let default = (QUAD_MB_PER_FILE, MODEL_MB_PER_FILE, FACE_MB_PER_FILE);
        let Some(spec) = self.knob("ARENA") else {
            return default;
        };
        match numbers::<usize>(spec)[..] {
            [quads, models, faces] => (quads.max(1), models.max(1), faces.max(1)),
            _ => {
                eprintln!("MCRS_ARENA needs three sizes in megabytes: quads,models,faces");
                default
            }
        }
    }

    pub(crate) fn flag(&self, name: &str, default: bool) -> bool {
        match self.knob(name).map(str::trim) {
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
    pub fn fullscreen(&self) -> bool {
        self.flag("FULLSCREEN", false)
    }

    /// `OPEN_TO_LAN=1` makes the integrated server listen on every interface and announce itself on
    /// the local network. The server has no authentication, so anyone on that network can join under
    /// any name, and whoever takes the host's name plays as the host, operator level included.
    #[cfg(all(feature = "singleplayer", not(target_family = "wasm")))]
    pub fn open_to_lan(&self) -> bool {
        self.flag("OPEN_TO_LAN", false)
    }

    /// `WORLD_SEED` is written signed, as the Java long it is, or unsigned; the router hashes the
    /// same bits either way.
    #[cfg(all(feature = "singleplayer", not(target_family = "wasm")))]
    pub fn world_seed(&self) -> u64 {
        let Some(raw) = self.knob("WORLD_SEED") else {
            return 0;
        };
        let raw = raw.trim();
        raw.parse::<i64>()
            .map(|seed| seed as u64)
            .or_else(|_| raw.parse::<u64>())
            .unwrap_or_else(|error| {
                eprintln!(
                    "MCRS_WORLD_SEED={raw} is not a number ({error}); generating with seed 0"
                );
                0
            })
    }

    /// `WORLD_PRESET` takes a short name (`normal`) or a namespaced one (`minecraft:normal`).
    #[cfg(all(feature = "singleplayer", not(target_family = "wasm")))]
    pub fn world_preset(&self) -> String {
        self.knob("WORLD_PRESET")
            .map(|name| name.trim().to_lowercase())
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| "normal".to_owned())
    }

    /// `NO_LIGHTING=1` turns light propagation off.
    #[cfg(all(feature = "singleplayer", not(target_family = "wasm")))]
    pub fn propagates_light(&self) -> bool {
        !self.flag("NO_LIGHTING", false)
    }

    /// `DEFAULT_GAMEMODE` is `survival`, `creative`, `adventure` or `spectator`, in any case; unset
    /// or anything else is creative.
    #[cfg(all(feature = "singleplayer", not(target_family = "wasm")))]
    pub fn default_game_mode(&self) -> mcrs_minecraft_server::GameMode {
        use mcrs_minecraft_server::GameMode;
        let Some(mode) = self.knob("DEFAULT_GAMEMODE") else {
            return GameMode::Creative;
        };
        match mode.trim().to_ascii_lowercase().as_str() {
            "survival" => GameMode::Survival,
            "creative" => GameMode::Creative,
            "adventure" => GameMode::Adventure,
            "spectator" => GameMode::Spectator,
            other => {
                eprintln!(
                    "MCRS_DEFAULT_GAMEMODE={other} is not a game mode; defaulting to creative"
                );
                GameMode::Creative
            }
        }
    }

    /// `SLOW_CHUNK_MS` is the latency above which the integrated server logs a column.
    #[cfg(all(feature = "singleplayer", not(target_family = "wasm")))]
    pub fn slow_column_threshold(&self) -> std::time::Duration {
        std::time::Duration::from_millis(
            self.knob("SLOW_CHUNK_MS")
                .and_then(|ms| ms.parse().ok())
                .unwrap_or(250),
        )
    }

    /// `GPU_HOT=<workgroups>` burns that many workgroups of arithmetic after the frame's own passes.
    /// A composited window presents at the display's rate and leaves the GPU idle most of the frame,
    /// and it clocks down while it waits: the same cull dispatch read 0.25 ms in a 60 Hz window and
    /// 0.11 ms fullscreen on a 120 Hz display, and the governor only lets go of the deadline once
    /// the GPU is saturated: 16384 workgroups did that on an M4 Max at 60 Hz, where the same cull
    /// read 0.058 ms. With the GPU held busy the pass timestamps are the code's own.
    pub fn gpu_hot(&self) -> Option<u32> {
        self.parsed(
            "GPU_HOT",
            |&workgroups| workgroups > 0,
            "expected a workgroup count above zero",
        )
    }

    /// The profiling overlay's own encoder timing and `PROBE=0` leave passes untimed, and
    /// `PROBE=1` times every frame. Otherwise a pass is timed only while the F3 overlay, the stats
    /// log or a scripted capture reads the figures.
    pub fn pass_timestamps(&self) -> bool {
        !self.overlay_times_encoders && self.knob("PROBE").is_none_or(|on| on != "0")
    }

    pub fn always_time_passes(&self) -> bool {
        self.knob("PROBE").is_some_and(|on| on == "1") || self.knob("CAPTURE").is_some()
    }

    /// `RESOLUTION=<width>x<height>` opens a window of exactly that many pixels instead of the
    /// fullscreen one, so a frame can be priced at a stated pixel count.
    pub fn resolution(&self) -> Option<(u32, u32)> {
        self.pair("RESOLUTION", 'x', "expected <width>x<height> in pixels")
    }

    /// Off by default so a frame time is readable: with vsync the frame reports the
    /// refresh interval no matter what the renderer costs.
    pub fn vsync(&self) -> bool {
        self.flag("VSYNC", false)
    }

    pub fn chunk_map(&self) -> bool {
        self.flag("CHUNK_MAP", false)
    }

    pub fn light_levels(&self) -> bool {
        self.flag("LIGHT_LEVELS", false)
    }

    /// Bevy's stock split gives async compute a quarter of the cores capped at four, which on a
    /// sixteen-core machine leaves meshing, column decode and the embedded server's lighting to
    /// share four threads while twelve idle. `ASYNC=<threads>` moves the cap.
    pub fn async_threads(&self) -> usize {
        self.knob("ASYNC")
            .and_then(|spec| spec.trim().parse().ok())
            .unwrap_or_else(async_compute_threads)
    }

    /// `MESH=<per frame>` and `MESH_FLIGHT=<jobs>` bound how fast the mesher may run: how many
    /// sections it admits in one frame and how many may be in flight at once.
    pub fn mesh_per_frame(&self) -> usize {
        self.knob("MESH")
            .and_then(|spec| spec.trim().parse().ok())
            .unwrap_or(MESH_PER_FRAME)
    }

    pub fn mesh_in_flight(&self) -> usize {
        self.knob("MESH_FLIGHT")
            .and_then(|spec| spec.trim().parse().ok())
            .unwrap_or(MESH_IN_FLIGHT)
    }

    /// Seconds between debug-screen lines written to the log, for a run whose
    /// window cannot be read.
    pub fn stats_interval(&self) -> Option<f32> {
        self.knob("STATS")
            .map(|spec| spec.trim().parse().unwrap_or(1.0))
    }

    /// `CAPTURE=<file>.png` waits for streaming to settle, writes one screenshot there and exits.
    pub fn capture_path(&self) -> Option<std::path::PathBuf> {
        let spec = self.knob("CAPTURE")?;
        let path = std::path::PathBuf::from(&spec);
        if path.extension().is_some_and(|extension| extension == "png") {
            Some(path)
        } else {
            reject("CAPTURE", spec, "expected a path ending in .png")
        }
    }

    pub fn screenshot_dir(&self) -> Option<std::path::PathBuf> {
        self.knob("SCREENSHOT_DIR").map(std::path::PathBuf::from)
    }

    /// Vanilla's "Brightness" slider, from 0 (Moody) through 0.5 (the default) to 1 (Bright).
    pub fn brightness(&self) -> f32 {
        self.parsed(
            "BRIGHTNESS",
            |value: &f32| (0.0..=1.0).contains(value),
            "a brightness from 0 to 1",
        )
        .unwrap_or(0.5)
    }

    /// `OCCLUSION=0` draws every group the frustum and the cave graph keep, without the depth
    /// pyramid test and its second pass.
    pub fn occlusion(&self) -> bool {
        self.flag("OCCLUSION", true)
    }

    pub fn cave(&self) -> bool {
        self.flag("CAVE", true)
    }

    pub(crate) fn parsed<T: std::str::FromStr>(
        &self,
        name: &str,
        check: impl FnOnce(&T) -> bool,
        expected: impl std::fmt::Display,
    ) -> Option<T> {
        let spec = self.knob(name)?;
        accepts(spec, check).or_else(|| reject(name, spec, expected))
    }

    pub(crate) fn pair<T: std::str::FromStr>(
        &self,
        name: &str,
        separator: char,
        expected: &str,
    ) -> Option<(T, T)> {
        let spec = self.knob(name)?;
        spec.split_once(separator)
            .and_then(|(a, b)| Some((a.trim().parse().ok()?, b.trim().parse().ok()?)))
            .or_else(|| reject(name, spec, expected))
    }

    /// `LOOK=<yaw>,<pitch>` aims the camera somewhere other than where the save
    /// left it, in Minecraft degrees.
    pub fn look_override(&self) -> Option<(f32, f32)> {
        self.pair("LOOK", ',', "expected <yaw>,<pitch> in degrees")
    }

    /// `POS=<x>,<y>,<z>` puts the player somewhere other than where the save left
    /// it, in blocks.
    pub fn position_override(&self) -> Option<bevy::math::DVec3> {
        let spec = self.knob("POS")?;
        let coordinates: Option<Vec<f64>> = spec
            .split(',')
            .map(|part| {
                part.trim()
                    .parse()
                    .ok()
                    .filter(|value: &f64| value.is_finite())
            })
            .collect();
        match coordinates.as_deref() {
            Some(&[x, y, z]) => Some(bevy::math::DVec3::new(x, y, z)),
            _ => reject("POS", spec, "expected <x>,<y>,<z> in blocks"),
        }
    }

    /// `TIME=<ticks>` pins every clock and stops them, so a scripted screenshot
    /// lands on the tick it asked for.
    pub fn frozen_time(&self) -> Option<i64> {
        self.parsed("TIME", |_| true, "expected a tick count")
    }

    /// `GUI_SCALE=<n>` pins the GUI scale; `0` or unset picks the largest scale
    /// that keeps 320x240 GUI units on screen, as vanilla's auto setting does.
    pub fn gui_scale(&self) -> u32 {
        self.parsed("GUI_SCALE", |_| true, "expected a whole GUI scale")
            .unwrap_or(0)
    }

    /// `SCREEN=inventory` opens that screen at start, so a capture is deterministic.
    pub fn opens_inventory(&self) -> bool {
        match self.knob("SCREEN") {
            None | Some("none") => false,
            Some("inventory") => true,
            Some(other) => reject("SCREEN", other, "expected none or inventory").unwrap_or(false),
        }
    }

    /// `CURSOR=<x>,<y>` pins the cursor in GUI units for the screens, in place of the pointer.
    pub fn gui_cursor(&self) -> Option<bevy::math::IVec2> {
        self.pair("CURSOR", ',', "expected <x>,<y> in GUI units")
            .map(|(x, y)| bevy::math::IVec2::new(x, y))
    }

    pub fn terrain(&self, limits: TerrainLimits) -> (Arc<Budget>, Uploads, CaveCull) {
        let (quad_mb, model_mb, face_mb) = self.arena_budget();
        let budget = Arc::new(Budget {
            quads: quad_mb * limits.arena_scale * 1_000_000 / QUAD_BYTES,
            models: model_mb * limits.arena_scale * 1_000_000 / MODEL_BYTES,
            faces: face_mb * limits.arena_scale * 1_000_000 / FACE_BYTES,
            groups: limits.groups,
            sections: limits.sections,
            upload: self.upload_budget(),
            tint_size: [limits.tint_span; 2],
        });

        bevy::log::info!(
            quad_mb = (budget.quads * QUAD_BYTES) / 1_000_000,
            model_mb = (budget.models * MODEL_BYTES) / 1_000_000,
            face_mb = (budget.faces * FACE_BYTES) / 1_000_000,
            "meshing the columns the server sends"
        );

        let cave = CaveCull::new(budget.sections, self.cave());
        (budget, Uploads::default(), cave)
    }
}

pub(crate) fn numbers<T: std::str::FromStr>(spec: &str) -> Vec<T> {
    spec.split(',')
        .filter_map(|n| n.trim().parse().ok())
        .collect()
}

#[cfg(all(feature = "singleplayer", not(target_family = "wasm")))]
pub fn integrated_bind_address(open_to_lan: bool) -> SocketAddr {
    let ip = if open_to_lan {
        Ipv4Addr::UNSPECIFIED
    } else {
        Ipv4Addr::LOCALHOST
    };
    SocketAddr::new(ip.into(), 0)
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
pub(crate) fn reject<T>(name: &str, value: &str, expected: impl std::fmt::Display) -> Option<T> {
    eprintln!("{}={value}: {expected}", spelled(name));
    std::process::exit(1);
}

#[cfg(target_family = "wasm")]
pub(crate) fn reject<T>(name: &str, value: &str, expected: impl std::fmt::Display) -> Option<T> {
    bevy::log::error!("{}={value}: {expected}", spelled(name));
    None
}

pub(crate) fn accepts<T: std::str::FromStr>(
    spec: &str,
    check: impl FnOnce(&T) -> bool,
) -> Option<T> {
    spec.trim().parse().ok().filter(check)
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

#[cfg(test)]
#[cfg(all(feature = "singleplayer", not(target_family = "wasm")))]
mod tests {
    use super::*;

    #[test]
    fn the_embedded_server_binds_loopback_unless_opened() {
        let closed = integrated_bind_address(false);
        assert_eq!(closed, "127.0.0.1:0".parse().unwrap());
        assert!(closed.ip().is_loopback());

        let opened = integrated_bind_address(true);
        assert_eq!(opened, "0.0.0.0:0".parse().unwrap());
        assert!(opened.ip().is_unspecified());
    }
}
