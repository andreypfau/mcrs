#[allow(dead_code, reason = "the block viewer uses the parts this example does not")]
#[path = "../block_viewer/model.rs"]
mod model;
#[allow(dead_code, reason = "the block viewer uses the parts this example does not")]
#[path = "../block_viewer/bake.rs"]
mod bake;
mod anim;
mod anvil;
mod arena;
mod atlas;
mod blocks;
mod camera;
mod cave;
mod config;
mod daylight;
mod mesh;
mod overlay;
mod pack;
mod probe;
mod render;
mod stream;
mod window;

use std::sync::Arc;

use bevy::camera::visibility::VisibilitySystems;
use bevy::diagnostic::{FrameTimeDiagnosticsPlugin, LogDiagnosticsPlugin};
use bevy::prelude::*;
use bevy::render::RenderPlugin;
use bevy::render::diagnostic::RenderDiagnosticsPlugin;
use bevy::render::render_resource::WgpuFeatures;
use bevy::render::settings::WgpuSettings;
use bevy::window::{PresentMode, WindowPosition};
use bevy::winit::{UpdateMode, WinitSettings};

use overlay::FrameStats;
use pack::RegionGrid;
use render::{Layout, TerrainPlugin, Uploads};

const DEFAULT_REGION: &str = "examples/anvil_region_viewer/r.0.0.mca";

const DEFAULT_WINDOW: usize = 2;

const BUDGET_FILES: usize = 4;
const GROUPS_PER_FILE: usize = 1 << 17;

const QUAD_BYTES: usize = pack::QUAD_WORDS * 4;
const MODEL_BYTES: usize = 4 * 3 * 4;
const FACE_BYTES: usize = 4;

fn main() {
    let mut args = std::env::args().skip(1);
    let path = args.next().unwrap_or_else(|| {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join(DEFAULT_REGION)
            .to_string_lossy()
            .into_owned()
    });
    let size = args
        .next()
        .and_then(|size| size.parse().ok())
        .unwrap_or(DEFAULT_WINDOW)
        .max(1);

    let window = match anvil::window(
        std::path::Path::new(&path),
        config::window_centre(),
        size,
    ) {
        Ok(window) => window,
        Err(error) => {
            eprintln!("cannot load {path}: {error}");
            std::process::exit(1);
        }
    };
    let layout = match layout(&window) {
        Ok(layout) => Arc::new(layout),
        Err(error) => {
            eprintln!("cannot start: {error}");
            std::process::exit(1);
        }
    };
    announce(&window, &layout);

    let cave = cave::CaveCull::new(
        cave::cave_grid(),
        layout.min_section,
        [
            window.regions[0] * anvil::REGION_CHUNKS,
            anvil::SECTIONS_Y,
            window.regions[1] * anvil::REGION_CHUNKS,
        ],
    );
    assert_eq!(
        cave.words(),
        layout.cave_words,
        "the sight-line bitset and the buffer it goes into have to be the same size"
    );
    let uploads = Uploads::default();
    let loader = stream::Loader::new(layout.clone(), uploads.clone(), window);

    App::new()
        .add_plugins(
            DefaultPlugins
                .set(RenderPlugin {
                    render_creation: WgpuSettings {
                        features: WgpuFeatures::TIMESTAMP_QUERY,
                        ..default()
                    }
                    .into(),
                    ..default()
                })
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: window::title(&path),
                        present_mode: PresentMode::AutoNoVsync,
                        mode: window::starting_mode(),
                        position: WindowPosition::default(),
                        ..default()
                    }),
                    ..default()
                })
                .disable::<bevy::pbr::PbrPlugin>(),
        )
        .insert_resource(WinitSettings {
            focused_mode: UpdateMode::Continuous,
            unfocused_mode: UpdateMode::Continuous,
        })
        .insert_resource(FrameStats::new())
        .insert_resource(config::drawn_streams())
        .insert_resource(config::raster_fraction())
        .insert_resource(config::sweep())
        .add_plugins((
            FrameTimeDiagnosticsPlugin::default(),
            RenderDiagnosticsPlugin,
            LogDiagnosticsPlugin {
                wait_duration: std::time::Duration::from_secs(2),
                ..default()
            },
        ))
        .add_plugins((TerrainPlugin(layout, uploads), daylight::DayCyclePlugin))
        .insert_resource(cave)
        .insert_resource(loader)
        .add_systems(Startup, (camera::spawn, overlay::spawn))
        .add_systems(
            Update,
            (
                window::place,
                window::toggle_fullscreen,
                window::screenshot,
                camera::orbit,
                stream::advance,
                overlay::frame_stats,
                cave::toggle,
                render::toggle_wireframe,
            ),
        )
        .add_systems(
            PostUpdate,
            cave::cave_cull.after(VisibilitySystems::UpdateFrusta),
        )
        .run();
}

fn layout(window: &anvil::Window) -> Result<Layout, String> {
    let chunks = anvil::REGION_CHUNKS;
    let grid = RegionGrid::covering([
        window.regions[0] * chunks,
        anvil::SECTIONS_Y,
        window.regions[1] * chunks,
    ]);
    let files = window.files.len().clamp(1, BUDGET_FILES);
    let (quad_mb, model_mb, face_mb) = config::arena_budget();
    let span = anvil::REGION_BLOCKS as u32;
    Ok(Layout {
        grid,
        min_section: [
            window.min_region[0] * chunks as i32,
            anvil::MIN_SECTION_Y,
            window.min_region[1] * chunks as i32,
        ],
        quad_capacity: quad_mb * files * 1_000_000 / QUAD_BYTES,
        model_capacity: model_mb * files * 1_000_000 / MODEL_BYTES,
        face_capacity: face_mb * files * 1_000_000 / FACE_BYTES,
        group_capacity: GROUPS_PER_FILE * files,
        cave_words: (cave::cave_grid().slots() + pack::SECTIONS_PER_RENDER_REGION).div_ceil(32),
        celestials: daylight::celestials()?,
        clouds: daylight::clouds()?,
        tint_origin: [
            window.min_region[0] * span as i32,
            window.min_region[1] * span as i32,
        ],
        tint_size: [
            window.regions[0] as u32 * span,
            window.regions[1] as u32 * span,
        ],
    })
}

fn announce(window: &anvil::Window, layout: &Layout) {
    println!(
        "{} region files at r.{}.{} and up, {}x{}x{} render regions, up to {} draws; \
         arenas hold {:.0} MB of quads, {:.0} MB of model vertices and {:.0} MB of block faces",
        window.files.len(),
        window.min_region[0],
        window.min_region[1],
        layout.grid.x,
        layout.grid.y,
        layout.grid.z,
        layout.max_draws(),
        (layout.quad_capacity * QUAD_BYTES) as f64 / 1e6,
        (layout.model_capacity * MODEL_BYTES) as f64 / 1e6,
        (layout.face_capacity * FACE_BYTES) as f64 / 1e6,
    );
}
