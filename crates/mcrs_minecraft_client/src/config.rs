use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

#[cfg(all(feature = "singleplayer", not(target_family = "wasm")))]
use std::net::{Ipv4Addr, SocketAddr};

use crate::cave::CaveCull;
use crate::options;
use bevy::app::App;
use bevy::ecs::resource::Resource;
use mcrs_minecraft_client_gui::scene::{GuiConfig, GuiPlugin};
use mcrs_minecraft_client_options::{ClientOptions, VIEW_DISTANCE_DEFAULT, VIEW_DISTANCE_RANGE};
use mcrs_minecraft_client_world::screen::{AccessibilityParent, Screen};
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

#[cfg(not(target_family = "wasm"))]
fn async_compute_threads() -> usize {
    (bevy::tasks::available_parallelism() / 2).max(4)
}

#[cfg(not(target_family = "wasm"))]
pub const IO_THREADS: usize = 2;
#[cfg(target_family = "wasm")]
pub const IO_THREADS: usize = 1;

#[cfg(target_family = "wasm")]
fn async_compute_threads() -> usize {
    1
}

#[cfg(not(target_family = "wasm"))]
const MESH_IN_FLIGHT: usize = 2048;
#[cfg(not(target_family = "wasm"))]
const MESH_PER_FRAME: usize = 2048;
#[cfg(target_family = "wasm")]
const MESH_IN_FLIGHT: usize = 128;
#[cfg(target_family = "wasm")]
const MESH_PER_FRAME: usize = 32;

pub const MAX_VIEW_DISTANCE: u8 = *VIEW_DISTANCE_RANGE.end();

#[derive(Resource, Clone, Debug, Default)]
pub struct Knobs {
    values: Arc<HashMap<String, String>>,
    options: ClientOptions,
    options_file: Option<PathBuf>,
    overlay_times_encoders: bool,
}

impl Knobs {
    pub fn new(
        values: impl IntoIterator<Item = (String, String)>,
        overlay_times_encoders: bool,
        default_options_file: Option<PathBuf>,
    ) -> Self {
        let mut map = HashMap::new();
        for (name, value) in values {
            map.entry(name).or_insert(value);
        }
        let options_file = map
            .get("OPTIONS")
            .map(PathBuf::from)
            .or(default_options_file);
        let mut knobs = Self {
            values: Arc::new(map),
            options: ClientOptions::default(),
            options_file,
            overlay_times_encoders,
        };
        knobs.options = knobs.start_options();
        knobs
    }

    fn start_options(&self) -> ClientOptions {
        let mut options = self.file_options();
        options.view_distance = self
            .parsed(
                "VIEW",
                |columns| VIEW_DISTANCE_RANGE.contains(columns),
                format_args!("expected a render distance from 2 to {MAX_VIEW_DISTANCE} columns"),
            )
            .or(options.view_distance);
        options.vsync = self.flag("VSYNC", options.vsync);
        options.fullscreen =
            self.flag("FULLSCREEN", options.fullscreen) && self.resolution().is_none();
        options
    }

    fn file_options(&self) -> ClientOptions {
        let Some(path) = &self.options_file else {
            return ClientOptions::default();
        };
        if self.knob("OPTIONS").is_none() && matches!(path.try_exists(), Ok(false)) {
            return ClientOptions::default();
        }
        match options::read(path) {
            Ok(options) => options,
            Err(error) => reject("OPTIONS", &path.display().to_string(), error).unwrap_or_default(),
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

    pub fn view_distance(&self) -> u8 {
        self.options.view_distance.unwrap_or(VIEW_DISTANCE_DEFAULT)
    }

    pub fn username(&self) -> String {
        self.knob("USERNAME").unwrap_or("Player").to_owned()
    }

    pub fn command_history(&self) -> Option<std::path::PathBuf> {
        self.knob("HISTORY").map(std::path::PathBuf::from)
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

    pub fn fullscreen(&self) -> bool {
        self.options.fullscreen
    }

    #[cfg(all(feature = "singleplayer", not(target_family = "wasm")))]
    pub fn open_to_lan(&self) -> bool {
        self.flag("OPEN_TO_LAN", false)
    }

    #[cfg(all(feature = "singleplayer", not(target_family = "wasm")))]
    pub fn server_settings(&self) -> mcrs_minecraft_server_settings::ServerSettings {
        mcrs_minecraft_server_settings::ServerSettings::read(
            |name| self.knob(name).map(str::to_owned),
            |error| eprintln!("{error}"),
        )
    }

    pub fn gpu_hot(&self) -> Option<u32> {
        self.parsed(
            "GPU_HOT",
            |&workgroups| workgroups > 0,
            "expected a workgroup count above zero",
        )
    }

    pub fn pass_timestamps(&self) -> bool {
        !self.overlay_times_encoders && self.knob("PROBE").is_none_or(|on| on != "0")
    }

    pub fn always_time_passes(&self) -> bool {
        self.knob("PROBE").is_some_and(|on| on == "1") || self.knob("CAPTURE").is_some()
    }

    pub fn resolution(&self) -> Option<(u32, u32)> {
        self.pair("RESOLUTION", 'x', "expected <width>x<height> in pixels")
    }

    pub fn vsync(&self) -> bool {
        self.options.vsync
    }

    pub fn chunk_map(&self) -> bool {
        self.flag("CHUNK_MAP", false)
    }

    pub fn light_levels(&self) -> bool {
        self.flag("LIGHT_LEVELS", false)
    }

    pub fn async_threads(&self) -> usize {
        self.knob("ASYNC")
            .and_then(|spec| spec.trim().parse().ok())
            .unwrap_or_else(async_compute_threads)
    }

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

    pub fn stats_interval(&self) -> Option<f32> {
        self.knob("STATS")
            .map(|spec| spec.trim().parse().unwrap_or(1.0))
    }

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

    pub fn options(&self) -> ClientOptions {
        self.options
    }

    pub fn options_file(&self) -> Option<PathBuf> {
        self.options_file.clone()
    }

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

    pub fn look_override(&self) -> Option<(f32, f32)> {
        self.pair("LOOK", ',', "expected <yaw>,<pitch> in degrees")
    }

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

    pub fn frozen_time(&self) -> Option<i64> {
        self.parsed("TIME", |_| true, "expected a tick count")
    }

    pub fn gui_scale(&self) -> Option<u32> {
        self.parsed("GUI_SCALE", |_| true, "expected a whole GUI scale")
    }

    pub fn gui(&self) -> GuiPlugin {
        GuiPlugin {
            config: GuiConfig {
                scale: 0,
                cursor: self.gui_cursor(),
                frozen_ticks: self.frozen_time(),
                advanced_tooltips: false,
            },
            screen: self.start_screen(),
        }
    }

    pub fn start_screen(&self) -> Screen {
        match self.knob("SCREEN") {
            None | Some("title") => Screen::Title,
            Some("none") => Screen::None,
            Some("inventory") => Screen::Menu,
            Some("pause") => Screen::Pause,
            Some("options") => Screen::Options,
            Some("video") => Screen::VideoSettings,
            Some("accessibility") => Screen::AccessibilitySettings(AccessibilityParent::Options),
            Some(other) => reject(
                "SCREEN",
                other,
                "expected title, none, inventory, pause, options, video or accessibility",
            )
            .unwrap_or_default(),
        }
    }

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

#[cfg(not(target_family = "wasm"))]
fn spelled(name: &str) -> String {
    format!("MCRS_{name}")
}

#[cfg(target_family = "wasm")]
fn spelled(name: &str) -> String {
    format!("?{}", name.to_ascii_lowercase())
}

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

    #[test]
    fn missing_options_file_gives_defaults_and_a_knob_wins() {
        let path =
            std::env::temp_dir().join(format!("mcrs-default-options-{}.json", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let missing = Knobs::new([], false, Some(path.clone()));
        assert_eq!(missing.options(), ClientOptions::default());
        assert_eq!(missing.options_file(), Some(path.clone()));

        std::fs::write(&path, r#"{"fov": 80, "vsync": false}"#).unwrap();
        let knobs = [("VSYNC".to_owned(), "1".to_owned())];
        let written = Knobs::new(knobs, false, Some(path.clone()));
        std::fs::remove_file(&path).unwrap();
        assert_eq!(written.options().fov, 80);
        assert!(written.vsync());
    }
}
