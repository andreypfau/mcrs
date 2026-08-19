//! Renders a whole Minecraft Anvil region file with a custom, culling-driven pipeline.
//!
//! ```text
//! cargo run --release --example anvil_region_viewer
//! cargo run --release --example anvil_region_viewer -- path/to/r.0.0.mca
//! cargo run --release --example anvil_region_viewer -- path/to/saves/world/region 3
//! ```
//!
//! Given a directory, a square window of region files is loaded around the region `ANVIL_CENTER`
//! names, two on a side unless a second argument says otherwise. A whole directory is deliberately
//! not an option: sixty-four region files are several gigabytes of geometry.
//!
//! Drag with the left mouse button to orbit, scroll to zoom, hold shift while dragging to pan.
//! Hold + or - to run the clock: the sky, the sun, the moon, the stars and the light on the terrain
//! all follow it, and the sky itself is drawn procedurally in the same pass the terrain is.
//! Press C to toggle cave culling, F9 to take the clouds out of the sky, F10 to draw every
//! triangle's edges in a colour derived from its texture, F11 for borderless fullscreen, which is
//! the only way to read a real frame rate on macOS, and F12 to save a PNG.
//!
//! Region files are parsed and meshed in the background and appear a render region at a time, so
//! the window opens on an empty sky rather than after a pause. Everything else is built around
//! never touching a quad again once it is down: blocks are baked per distinct block state rather
//! than per block, full cubes are greedy-merged into twelve-byte quads, and every frame the GPU
//! alone decides what to draw. There is no `Mesh` asset, no entity per section, and one indirect
//! draw call per stream per render region.

// Shared verbatim with the block viewer rather than copied: the model resolver and the face bakery
// are the pieces this example most needs to stay identical to the single-block reference.
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
mod cave;
mod mesh;
mod pack;
mod render;
mod sky;
mod stream;

use std::fmt::Write as _;
use std::sync::Arc;

use bevy::camera::visibility::VisibilitySystems;
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::diagnostic::{FrameTimeDiagnosticsPlugin, LogDiagnosticsPlugin};
use bevy::render::RenderPlugin;
use bevy::render::diagnostic::RenderDiagnosticsPlugin;
use bevy::render::render_resource::WgpuFeatures;
use bevy::render::settings::WgpuSettings;
use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll, MouseScrollUnit};
use bevy::prelude::*;
use bevy::render::view::Msaa;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use bevy::window::{MonitorSelection, PresentMode, WindowMode};
use bevy::winit::{UpdateMode, WinitSettings};

use pack::RegionGrid;
use render::{DrawnTriangles, Layout, TerrainPlugin, Uploads, Wireframe};

const DEFAULT_REGION: &str = "examples/anvil_region_viewer/r.0.0.mca";

/// Region files on a side when a directory is given. Two is enough to show terrain running
/// unbroken across a region seam; the sixty-four files of a real world would be several gigabytes
/// of geometry.
const DEFAULT_WINDOW: usize = 2;

// How much room each region file gets in the two geometry arenas, in megabytes.
//
// Measured off this world, where the heaviest file needs 38 MB of greedy quads and 158 MB of model
// vertices, plus the third the arena's size classes cost in rounding and a little over for a
// denser file. `ANVIL_ARENA=quads,models` overrides both. A region that does not fit is dropped
// and counted rather than drawn half-written.
const QUAD_MB_PER_FILE: usize = 64;
const MODEL_MB_PER_FILE: usize = 208;

/// Region files the arenas are sized for, however many the window covers.
///
/// The budget is the knob and the view is what follows from it, not the other way round: a wider
/// window does not get a wider arena, it gets the same one and has to keep only what fits in it.
/// Four files' worth is what the default window needs, and a single file still only pays for one.
const BUDGET_FILES: usize = 4;
/// Culling groups per file. The heaviest of these four holds seventy thousand, and rounding takes
/// its share of this arena too.
const GROUPS_PER_FILE: usize = 1 << 17;

/// Bytes one greedy quad and one model quad take in their arenas.
const QUAD_BYTES: usize = pack::QUAD_WORDS * 4;
const MODEL_BYTES: usize = 4 * 3 * 4;

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

    let window = match anvil::window(std::path::Path::new(&path), window_centre(), size) {
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
    println!(
        "{} region files at r.{}.{} and up, {}x{}x{} render regions, up to {} draws; \
         arenas hold {:.0} MB of quads and {:.0} MB of model vertices",
        window.files.len(),
        window.min_region[0],
        window.min_region[1],
        layout.grid.x,
        layout.grid.y,
        layout.grid.z,
        layout.max_draws(),
        (layout.quad_capacity * QUAD_BYTES) as f64 / 1e6,
        (layout.model_capacity * MODEL_BYTES) as f64 / 1e6,
    );

    let cave = cave::CaveCull::new(
        vec![mesh::CONNECT_ALL; layout.grid.slots()],
        layout.grid,
        [
            window.regions[0] * anvil::REGION_CHUNKS,
            anvil::SECTIONS_Y,
            window.regions[1] * anvil::REGION_CHUNKS,
        ],
        layout.min_section,
    );
    assert_eq!(
        cave.words(),
        layout.cave_words,
        "the sight-line bitset and the buffer it goes into have to be the same size"
    );
    let uploads = Uploads::default();
    let loader = stream::Loader::new(layout.clone(), uploads.clone(), window);

    App::new()
        .add_plugins(DefaultPlugins
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
                    title: title_base(&path),
                    // Asks for no vsync so the counter reflects the renderer rather than the
                    // display. This reaches Metal as `displaySyncEnabled = false`, but a composited
                    // macOS window still gets its drawables recycled by the window server at the
                    // refresh rate, so `nextDrawable` blocks for the rest of the frame regardless.
                    // Fullscreen (F11) takes the window off that path and is what actually uncaps.
                    present_mode: PresentMode::AutoNoVsync,
                    // The same swap F11 does, at startup, so a rate can be measured from a
                    // terminal without a human at the window.
                    mode: match std::env::var("ANVIL_FULLSCREEN") {
                        Ok(_) => WindowMode::BorderlessFullscreen(MonitorSelection::Current),
                        Err(_) => WindowMode::Windowed,
                    },
                    ..default()
                }),
                ..default()
            }))
        // Bevy throttles an unfocused window to 60 Hz to save power, which silently pins the
        // counter to exactly that and hides what the renderer is really doing.
        .insert_resource(WinitSettings {
            focused_mode: UpdateMode::Continuous,
            unfocused_mode: UpdateMode::Continuous,
        })
        .insert_resource(FrameStats::new())
        .insert_resource(Sweep(
            std::env::var("ANVIL_SWEEP")
                .ok()
                .and_then(|speed| speed.parse().ok())
                .unwrap_or(0.0),
        ))
        .add_plugins((
            FrameTimeDiagnosticsPlugin::default(),
            // Per-pass timings, which stay meaningful while the window is capped and the wall
            // clock says nothing about how much headroom the culling and draw passes leave.
            RenderDiagnosticsPlugin,
            LogDiagnosticsPlugin {
                wait_duration: std::time::Duration::from_secs(2),
                ..default()
            },
        ))
        .add_plugins((TerrainPlugin(layout, uploads), sky::DayCyclePlugin))
        .insert_resource(cave)
        .insert_resource(loader)
        .add_systems(Startup, (spawn_camera, spawn_overlay))
        .add_systems(Update, stream::advance)
        .add_systems(Update, orbit)
        .add_systems(Update, frame_stats)
        .add_systems(Update, screenshot)
        .add_systems(Update, toggle_fullscreen)
        .add_systems(Update, toggle_wireframe)
        .add_systems(Update, cave::toggle)
        .add_systems(
            PostUpdate,
            cave::cave_cull.after(VisibilitySystems::UpdateFrusta),
        )
        .run();
}

/// The shape of the world, which the window settles before a single file is read.
fn layout(window: &anvil::Window) -> Result<Layout, String> {
    let chunks = anvil::REGION_CHUNKS;
    let grid = RegionGrid::covering([
        window.regions[0] * chunks,
        anvil::SECTIONS_Y,
        window.regions[1] * chunks,
    ]);
    // Against the files that are really there rather than the slots of the window, and capped: a
    // corner of the world with nothing in it should not cost a gigabyte of arena, and a wide window
    // should not quietly buy itself more room than the budget allows.
    let files = window.files.len().clamp(1, BUDGET_FILES);
    let (quad_mb, model_mb) = arena_budget();
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
        group_capacity: GROUPS_PER_FILE * files,
        cave_words: grid.slots().div_ceil(32),
        celestials: sky::celestials()?,
        clouds: sky::clouds()?,
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

/// `ANVIL_ARENA=quads,models` sets how many megabytes a region file gets in each arena, which is
/// what makes it checkable that the loader still fills a frame out of half the room.
fn arena_budget() -> (usize, usize) {
    let default = (QUAD_MB_PER_FILE, MODEL_MB_PER_FILE);
    let Ok(spec) = std::env::var("ANVIL_ARENA") else {
        return default;
    };
    let numbers: Vec<usize> = spec.split(',').filter_map(|n| n.trim().parse().ok()).collect();
    match numbers[..] {
        [quads, models] => (quads.max(1), models.max(1)),
        _ => {
            eprintln!("ANVIL_ARENA needs two sizes in megabytes: quads,models");
            default
        }
    }
}

/// `ANVIL_CENTER=x,z` names the region the loaded window is centred on. Default is the origin,
/// which for an even window straddles it and so puts negative region coordinates in the frame.
fn window_centre() -> [i32; 2] {
    let Ok(spec) = std::env::var("ANVIL_CENTER") else {
        return [0, 0];
    };
    let numbers: Vec<i32> = spec.split(',').filter_map(|n| n.trim().parse().ok()).collect();
    match numbers[..] {
        [x, z] => [x, z],
        _ => {
            eprintln!("ANVIL_CENTER needs two region coordinates: x,z");
            [0, 0]
        }
    }
}

/// Frame times over the last second, kept in a fixed ring so the counter itself never allocates
/// and never grows no matter how fast the renderer runs.
#[derive(Resource)]
struct FrameStats {
    times: Box<[f32; FrameStats::CAPACITY]>,
    sorted: Box<[f32; FrameStats::CAPACITY]>,
    /// Frames this second, which can exceed the ring and still give a correct rate.
    frames: u32,
    written: usize,
    elapsed: f32,
}

impl FrameStats {
    const CAPACITY: usize = 4096;

    fn new() -> Self {
        Self {
            times: Box::new([0.0; Self::CAPACITY]),
            sorted: Box::new([0.0; Self::CAPACITY]),
            frames: 0,
            written: 0,
            elapsed: 0.0,
        }
    }
}

fn title_base(path: &str) -> String {
    let name = std::path::Path::new(path)
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string());
    format!("anvil region viewer — {name}")
}

/// Reports the rate, the triangles culling let through, and the two tail percentiles once a second.
/// An average alone hides exactly the thing worth seeing while orbiting: the occasional frame where
/// culling lets far more through.
fn frame_stats(
    time: Res<Time>,
    mut stats: ResMut<FrameStats>,
    triangles: Res<DrawnTriangles>,
    cave: Res<cave::CaveCull>,
    loader: Res<stream::Loader>,
    day: Res<sky::TimeOfDay>,
    overlay: Single<&mut Text>,
    // The frame rate is meaningless without the pixel count behind it: this window opens on
    // whichever display is current, and the two here differ enough to change the number outright.
    window: Single<&Window>,
) {
    let delta = time.delta_secs();
    if delta <= 0.0 {
        return;
    }
    stats.elapsed += delta;
    stats.frames += 1;
    let slot = stats.written % FrameStats::CAPACITY;
    stats.times[slot] = delta * 1000.0;
    stats.written += 1;

    if stats.elapsed < 1.0 {
        return;
    }

    // Reborrowed once so the two arrays are seen as disjoint fields rather than two derefs of
    // the same resource handle.
    let stats = &mut *stats;
    let samples = stats.written.min(FrameStats::CAPACITY);
    stats.sorted[..samples].copy_from_slice(&stats.times[..samples]);
    stats.sorted[..samples].sort_unstable_by(f32::total_cmp);
    let fps = stats.frames as f32 / stats.elapsed;
    let p95 = stats.sorted[percentile_index(samples, 0.95)];
    let p99 = stats.sorted[percentile_index(samples, 0.99)];

    // Written straight into the component's own buffer, so the once-a-second update reuses the
    // allocation instead of building a throwaway string, and the text is only re-laid-out then.
    let mut overlay = overlay;
    overlay.0.clear();
    let _ = write!(
        overlay.0,
        "{fps:.0} fps @ {}x{}   {} tris   p95 {p95:.1} ms   p99 {p99:.1} ms",
        window.resolution.physical_width(),
        window.resolution.physical_height(),
        triangles.get(),
    );
    if cave.enabled {
        let _ = write!(overlay.0, "   cave {} sections", cave.reached());
    } else {
        overlay.0.push_str("   cave off");
    }
    // How full the arena is, always rather than only while loading: a figure that saws is what
    // says the loader is thrashing on a threshold.
    let status = loader.status();
    let _ = write!(
        overlay.0,
        "   arena {:.0}/{:.0}%",
        status.quads * 100.0,
        status.models * 100.0,
    );
    let _ = write!(
        overlay.0,
        "   {}/{} regions",
        status.regions, status.regions_total,
    );
    if status.files < status.files_total {
        let _ = write!(overlay.0, "   loading {}/{} files", status.files, status.files_total);
    }
    if status.evicted > 0 {
        let _ = write!(overlay.0, "   {} evicted", status.evicted);
    }
    let (hour, minute) = day.clock();
    let _ = write!(overlay.0, "   {hour:02}:{minute:02}");
    // The `ANVIL_SCREENSHOT` shot goes off thirty frames after loading settles, so the overlay is
    // usually written by then. The numbers reach stdout either way.
    info!("{}", overlay.0);

    stats.frames = 0;
    stats.written = 0;
    stats.elapsed = 0.0;
}

/// Press F10 to draw the triangle edges, which show the real polygon count, where greedy merging
/// landed, and which texture each face pulls from.
fn toggle_wireframe(keys: Res<ButtonInput<KeyCode>>, mut wireframe: ResMut<Wireframe>) {
    if keys.just_pressed(KeyCode::F10) {
        wireframe.0 = !wireframe.0;
    }
}

/// Press F11 to swap between a window and borderless fullscreen. A composited window on macOS is
/// pinned to the display refresh no matter what present mode is asked for; fullscreen is what lets
/// the frame rate show the renderer instead.
fn toggle_fullscreen(keys: Res<ButtonInput<KeyCode>>, window: Single<&mut Window>) {
    if !keys.just_pressed(KeyCode::F11) {
        return;
    }
    let mut window = window;
    window.mode = match window.mode {
        WindowMode::Windowed => WindowMode::BorderlessFullscreen(MonitorSelection::Current),
        _ => WindowMode::Windowed,
    };
}

#[inline]
fn percentile_index(samples: usize, fraction: f32) -> usize {
    (((samples - 1) as f32) * fraction).round() as usize
}

/// Press F12 to write a PNG of the current view. Setting `ANVIL_SCREENSHOT` shoots one
/// automatically once the window has finished filling and exits, which is what makes the renderer
/// checkable from a terminal without a human at it.
fn screenshot(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    loader: Res<stream::Loader>,
    mut frames: Local<u32>,
    mut settled: Local<u32>,
) {
    *frames += 1;
    // Counted from when the loader went quiet rather than from the start: the window fills in the
    // background now, and a shot taken on a fixed frame catches whatever happened to be up.
    if loader.done() {
        *settled += 1;
    }
    let auto = std::env::var("ANVIL_SCREENSHOT").ok();
    let path = match (&auto, keys.just_pressed(KeyCode::F12)) {
        (Some(path), _) if *settled == 30 => path.clone(),
        (_, true) => "anvil_region_viewer.png".to_string(),
        _ => {
            if auto.is_some() && *frames > 3600 {
                commands.write_message(AppExit::Success);
            }
            return;
        }
    };
    commands.spawn(Screenshot::primary_window()).observe(save_to_disk(path));
}

/// `ANVIL_SWEEP=<blocks a second>` walks the camera along x on its own.
///
/// A pinned view is what a frame rate has to be compared at, but the faults streaming introduces
/// only appear while the view is moving: a region arriving mid-frame, a seam crossed, a region
/// giving its room to a nearer one. A scripted walk makes those repeatable from a terminal.
#[derive(Resource)]
struct Sweep(f32);

#[derive(Component)]
struct Orbit {
    yaw: f32,
    pitch: f32,
    radius: f32,
    target: Vec3,
}

fn spawn_camera(mut commands: Commands) {
    let orbit = starting_orbit();
    commands.spawn((
        Camera3d::default(),
        // No `Hdr` component: the terrain pipeline targets the default swap-chain format, and the
        // vanilla shade values are already display-referred so there is nothing to tone map.
        Tonemapping::None,
        // The terrain pipeline is built for a single sample; matching it here avoids specialising
        // the whole pipeline set on a setting this viewer has no use for.
        Msaa::Off,
        Projection::Perspective(PerspectiveProjection {
            far: 4000.0,
            ..default()
        }),
        Transform::default(),
        orbit,
    ));
}

/// The counter lives in the corner rather than the window title because fullscreen, which is the
/// only mode that reports an honest frame rate on macOS, hides the title bar.
fn spawn_overlay(mut commands: Commands) {
    commands.spawn((
        Text::new(""),
        TextFont {
            font_size: FontSize::Px(16.0),
            ..default()
        },
        TextColor(Color::WHITE),
        TextShadow::default(),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(8.0),
            left: Val::Px(8.0),
            ..default()
        },
    ));
}

/// `ANVIL_VIEW=yaw,pitch,radius,x,y,z` pins the camera where the run starts, so a frame rate can
/// be compared between builds rather than between two hand-held views that were never the same.
fn starting_orbit() -> Orbit {
    let default = Orbit {
        yaw: 0.8,
        pitch: 0.6,
        radius: 420.0,
        target: Vec3::new(256.0, 64.0, 256.0),
    };
    let Ok(spec) = std::env::var("ANVIL_VIEW") else {
        return default;
    };
    let numbers: Vec<f32> = spec.split(',').filter_map(|n| n.trim().parse().ok()).collect();
    let [yaw, pitch, radius, x, y, z] = numbers[..] else {
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

fn orbit(
    buttons: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    motion: Res<AccumulatedMouseMotion>,
    scroll: Res<AccumulatedMouseScroll>,
    time: Res<Time>,
    sweep: Res<Sweep>,
    camera: Single<(&mut Orbit, &mut Transform)>,
) {
    let (mut orbit, mut transform) = camera.into_inner();
    orbit.target.x += sweep.0 * time.delta_secs();
    let panning = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    if buttons.pressed(MouseButton::Left) {
        if panning {
            let right = transform.right() * -motion.delta.x * orbit.radius * 0.002;
            let up = transform.up() * motion.delta.y * orbit.radius * 0.002;
            orbit.target += right + up;
        } else {
            orbit.yaw -= motion.delta.x * 0.005;
            orbit.pitch = (orbit.pitch - motion.delta.y * 0.005).clamp(-1.54, 1.54);
        }
    }
    let zoom = match scroll.unit {
        MouseScrollUnit::Line => scroll.delta.y,
        MouseScrollUnit::Pixel => scroll.delta.y / MouseScrollUnit::SCROLL_UNIT_CONVERSION_FACTOR,
    };
    orbit.radius = (orbit.radius * (1.0 - zoom * 0.08)).clamp(2.0, 2000.0);

    let rotation = Quat::from_euler(EulerRot::YXZ, orbit.yaw, -orbit.pitch, 0.0);
    let target = orbit.target;
    *transform = Transform::from_translation(target + rotation * (Vec3::Z * orbit.radius))
        .looking_at(target, Vec3::Y);
}
