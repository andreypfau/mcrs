//! Renders a whole Minecraft Anvil region file with a custom, culling-driven pipeline.
//!
//! ```text
//! cargo run --release --example anvil_region_viewer
//! cargo run --release --example anvil_region_viewer -- path/to/r.0.0.mca
//! ```
//!
//! Drag with the left mouse button to orbit, scroll to zoom, hold shift while dragging to pan.
//! Press C to toggle cave culling, F10 to draw every triangle's edges in a colour derived from its
//! texture, F11 for borderless fullscreen, which is the only way to read a real frame rate on macOS,
//! and F12 to save a PNG.
//!
//! The region is static, so the whole pipeline is built around loading once and never touching the
//! geometry again: blocks are baked per distinct block state rather than per block, full cubes are
//! greedy-merged into eight-byte quads, and every frame the GPU alone decides what to draw. There
//! is no `Mesh` asset, no entity per section, and one indirect draw call per pass.

// Shared verbatim with the block viewer rather than copied: the model resolver and the face bakery
// are the pieces this example most needs to stay identical to the single-block reference.
#[allow(dead_code, reason = "the block viewer uses the parts this example does not")]
#[path = "../block_viewer/model.rs"]
mod model;
#[allow(dead_code, reason = "the block viewer uses the parts this example does not")]
#[path = "../block_viewer/bake.rs"]
mod bake;
mod anvil;
mod atlas;
mod blocks;
mod cave;
mod mesh;
mod pack;
mod render;

use std::fmt::Write as _;
use std::sync::Arc;
use std::time::Instant;

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

use render::{DrawnTriangles, Geometry, TerrainPlugin, Wireframe};

const DEFAULT_REGION: &str = "examples/anvil_region_viewer/r.0.0.mca";

fn main() {
    let path = std::env::args().nth(1).unwrap_or_else(|| {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join(DEFAULT_REGION)
            .to_string_lossy()
            .into_owned()
    });

    let (geometry, cave) = match load(std::path::Path::new(&path)) {
        Ok(loaded) => loaded,
        Err(error) => {
            eprintln!("cannot load {path}: {error}");
            std::process::exit(1);
        }
    };
    let geometry = Arc::new(geometry);

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
        .add_plugins(TerrainPlugin(geometry))
        .insert_resource(ClearColor(Color::srgb(0.55, 0.70, 0.94)))
        .insert_resource(cave)
        .add_systems(Startup, (spawn_camera, spawn_overlay))
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

fn load(path: &std::path::Path) -> Result<(Geometry, cave::CaveCull), String> {
    let started = Instant::now();
    let region = anvil::load(path)?;
    let parsed = started.elapsed();

    let started = Instant::now();
    let catalog = blocks::build(&region);
    let baked = started.elapsed();
    // Plain printing, not `warn!`: loading happens before the app — and therefore the log
    // subscriber — exists.
    for failure in &catalog.failures {
        println!("skipping {failure}");
    }

    let started = Instant::now();
    let mut region_mesh = mesh::build(&region, &catalog);
    let meshed = started.elapsed();

    println!(
        "{} sections, {} block states ({} unrenderable), {} sprites; parsed in {parsed:.2?}, baked in {baked:.2?}, meshed in {meshed:.2?}",
        region.non_empty_sections(),
        region.states.len(),
        catalog.failures.len(),
        catalog.sprites.len(),
    );
    let grid = region_mesh.grid;
    for (index, array) in catalog.sprites.arrays().iter().enumerate() {
        println!(
            "  sprite array {index}: {:>4} sprites at {}x{}",
            array.sprites.len(),
            array.size,
            array.size,
        );
    }
    println!(
        "{} greedy quads + {} model quads in {} culling groups; \
         {}x{}x{} render regions cost {} draws",
        region_mesh.greedy_quads(),
        region_mesh.model_quads(),
        region_mesh.groups.len(),
        grid.x,
        grid.y,
        grid.z,
        region_mesh.draws.len(),
    );
    for (stream, span) in region_mesh.streams.iter().enumerate() {
        println!(
            "  {:<22} {:>9} quads in {:>6} groups",
            mesh::STREAM_NAMES[stream], span.quad_count, span.group_count,
        );
    }

    let closed = region_mesh
        .connectivity
        .iter()
        .filter(|mask| **mask == 0)
        .count();
    println!("{closed} sections are fully closed to sight lines");

    // A zero-length storage buffer is not a legal binding, so every arena keeps at least one entry.
    if region_mesh.simple.is_empty() {
        region_mesh.simple.push(0);
    }
    if region_mesh.complex.is_empty() {
        region_mesh.complex.resize(3, 0);
    }
    if region_mesh.groups.is_empty() {
        region_mesh.groups.push(mesh::Group::default());
    }

    let sprites = catalog.sprites;
    let cave = cave::CaveCull::new(
        region_mesh.connectivity,
        region_mesh.grid,
        [anvil::REGION_CHUNKS, region.sections_y, anvil::REGION_CHUNKS],
        region.min_section_y,
    );
    Ok((
        Geometry {
            simple: region_mesh.simple,
            complex: region_mesh.complex,
            groups: region_mesh.groups,
            draws: region_mesh.draws,
            cave_words: cave.words(),
            atlases: sprites
                .arrays()
                .iter()
                .map(|array| render::Atlas {
                    size: array.size,
                    layers: array.layers(),
                    mips: array.mip_chain(),
                })
                .collect(),
            tint_map: blocks::tint_map(&region, &catalog.tints),
        },
        cave,
    ))
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
    // The `ANVIL_SCREENSHOT` shot is taken on frame 30, and the overlay is first written only after
    // a second of accumulated time, so it is blank in the PNG. The numbers do reach stdout.
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
/// automatically a few frames in and exits, which is what makes the renderer checkable from a
/// terminal without a human at the window.
fn screenshot(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    mut frames: Local<u32>,
) {
    *frames += 1;
    let auto = std::env::var("ANVIL_SCREENSHOT").ok();
    let path = match (&auto, keys.just_pressed(KeyCode::F12)) {
        (Some(path), _) if *frames == 30 => path.clone(),
        (_, true) => "anvil_region_viewer.png".to_string(),
        _ => {
            if auto.is_some() && *frames > 600 {
                commands.write_message(AppExit::Success);
            }
            return;
        }
    };
    commands.spawn(Screenshot::primary_window()).observe(save_to_disk(path));
}

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
    camera: Single<(&mut Orbit, &mut Transform)>,
) {
    let (mut orbit, mut transform) = camera.into_inner();
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
