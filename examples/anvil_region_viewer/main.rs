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
mod probe;
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
use bevy::window::{Monitor, MonitorSelection, PresentMode, VideoModeSelection, WindowMode, WindowPosition};
use bevy::winit::{UpdateMode, WinitSettings};

use pack::RegionGrid;
use render::{DrawnTriangles, Layout, TerrainPlugin, Uploads, Wireframe};

const DEFAULT_REGION: &str = "examples/anvil_region_viewer/r.0.0.mca";

const DEFAULT_WINDOW: usize = 2;

const QUAD_MB_PER_FILE: usize = 32;
const MODEL_MB_PER_FILE: usize = 208;
const FACE_MB_PER_FILE: usize = 40;

const BUDGET_FILES: usize = 4;
const GROUPS_PER_FILE: usize = 1 << 17;

const QUAD_BYTES: usize = pack::QUAD_WORDS * 4;
const MODEL_BYTES: usize = 4 * 3 * 4;
const FACE_BYTES: usize = 4;

fn chosen_monitor(monitors: &Query<(Entity, &Monitor)>) -> MonitorSelection {
    let Ok(spec) = std::env::var("ANVIL_MONITOR") else {
        return MonitorSelection::Current;
    };
    if spec == "primary" {
        return MonitorSelection::Primary;
    }
    if let Ok(index) = spec.parse() {
        return MonitorSelection::Index(index);
    }
    let wanted = spec.to_lowercase();
    let known: Vec<String> = monitors.iter().map(|(_, monitor)| describe(monitor)).collect();
    let found = monitors
        .iter()
        .find(|(_, monitor)| describe(monitor).to_lowercase().contains(&wanted));
    match found {
        Some((entity, monitor)) => {
            info!("ANVIL_MONITOR={spec} picked {} out of {known:?}", describe(monitor));
            MonitorSelection::Entity(entity)
        }
        None => {
            warn!("no display matches ANVIL_MONITOR={spec}; this machine has {known:?}");
            MonitorSelection::Current
        }
    }
}

fn describe(monitor: &Monitor) -> String {
    format!(
        "{} {}x{}",
        monitor.name.as_deref().unwrap_or("display"),
        monitor.physical_width,
        monitor.physical_height,
    )
}

fn fullscreen_mode(monitor: MonitorSelection) -> Option<WindowMode> {
    match std::env::var("ANVIL_FULLSCREEN").as_deref() {
        Ok("exclusive") => Some(WindowMode::Fullscreen(
            monitor,
            VideoModeSelection::Current,
        )),
        Ok(_) => Some(WindowMode::BorderlessFullscreen(monitor)),
        Err(_) => None,
    }
}

fn place_window(
    window: Single<&mut Window>,
    monitors: Query<(Entity, &Monitor)>,
    mut placed: Local<bool>,
) {
    if *placed || monitors.is_empty() {
        return;
    }
    let monitor = chosen_monitor(&monitors);
    let mut window = window;
    match fullscreen_mode(monitor) {
        Some(mode) => window.mode = mode,
        None => window.position = WindowPosition::Centered(monitor),
    }
    *placed = true;
}

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
                    present_mode: PresentMode::AutoNoVsync,
                    mode: match fullscreen_mode(MonitorSelection::Current) {
                        Some(_) => WindowMode::BorderlessFullscreen(MonitorSelection::Current),
                        None => WindowMode::Windowed,
                    },
                    position: WindowPosition::default(),
                    ..default()
                }),
                ..default()
            })
            .disable::<bevy::pbr::PbrPlugin>())
        .insert_resource(WinitSettings {
            focused_mode: UpdateMode::Continuous,
            unfocused_mode: UpdateMode::Continuous,
        })
        .insert_resource(FrameStats::new())
        .insert_resource(drawn_streams())
        .insert_resource(raster_fraction())
        .insert_resource(Sweep(
            std::env::var("ANVIL_SWEEP")
                .ok()
                .and_then(|speed| speed.parse().ok())
                .unwrap_or(0.0),
        ))
        .add_plugins((
            FrameTimeDiagnosticsPlugin::default(),
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
        .add_systems(Update, place_window)
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

fn layout(window: &anvil::Window) -> Result<Layout, String> {
    let chunks = anvil::REGION_CHUNKS;
    let grid = RegionGrid::covering([
        window.regions[0] * chunks,
        anvil::SECTIONS_Y,
        window.regions[1] * chunks,
    ]);
    let files = window.files.len().clamp(1, BUDGET_FILES);
    let (quad_mb, model_mb, face_mb) = arena_budget();
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
        cave_words: (cave::cave_grid().slots() + pack::SECTIONS_PER_RENDER_REGION)
            .div_ceil(32),
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

fn arena_budget() -> (usize, usize, usize) {
    let default = (QUAD_MB_PER_FILE, MODEL_MB_PER_FILE, FACE_MB_PER_FILE);
    let Ok(spec) = std::env::var("ANVIL_ARENA") else {
        return default;
    };
    let numbers: Vec<usize> = spec.split(',').filter_map(|n| n.trim().parse().ok()).collect();
    match numbers[..] {
        [quads, models, faces] => (quads.max(1), models.max(1), faces.max(1)),
        _ => {
            eprintln!("ANVIL_ARENA needs three sizes in megabytes: quads,models,faces");
            default
        }
    }
}

fn drawn_streams() -> render::Streams {
    let Ok(spec) = std::env::var("ANVIL_STREAMS") else {
        return render::Streams::default();
    };
    let mut mask = 0;
    for name in spec.split(',') {
        match name.trim().parse::<u32>() {
            Ok(stream) if (stream as usize) < mesh::STREAMS => mask |= 1 << stream,
            _ => eprintln!("ANVIL_STREAMS takes stream numbers 0..{}", mesh::STREAMS - 1),
        }
    }
    render::Streams(mask)
}

fn raster_fraction() -> render::Raster {
    let Ok(spec) = std::env::var("ANVIL_RASTER") else {
        return render::Raster::default();
    };
    match spec.trim().parse::<f32>() {
        Ok(fraction) if (0.0..=1.0).contains(&fraction) && fraction > 0.0 => {
            render::Raster(fraction)
        }
        _ => {
            eprintln!("ANVIL_RASTER takes a fraction between 0 and 1");
            render::Raster::default()
        }
    }
}

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

#[derive(Resource)]
struct FrameStats {
    times: Box<[f32; FrameStats::CAPACITY]>,
    sorted: Box<[f32; FrameStats::CAPACITY]>,
    frames: u32,
    written: usize,
    elapsed: f32,
    line: String,
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
            line: String::new(),
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

fn frame_stats(
    time: Res<Time>,
    mut stats: ResMut<FrameStats>,
    triangles: Res<DrawnTriangles>,
    gpu: Res<probe::GpuTimings>,
    streams: Res<render::Streams>,
    cave: Res<cave::CaveCull>,
    loader: Res<stream::Loader>,
    day: Res<sky::TimeOfDay>,
    overlay: Option<Single<&mut Text>>,
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

    let stats = &mut *stats;
    let samples = stats.written.min(FrameStats::CAPACITY);
    stats.sorted[..samples].copy_from_slice(&stats.times[..samples]);
    stats.sorted[..samples].sort_unstable_by(f32::total_cmp);
    let fps = stats.frames as f32 / stats.elapsed;
    let p95 = stats.sorted[percentile_index(samples, 0.95)];
    let p99 = stats.sorted[percentile_index(samples, 0.99)];

    let line = &mut stats.line;
    line.clear();
    let _ = write!(
        line,
        "{fps:.0} fps @ {}x{}   {} tris   p95 {p95:.1} ms   p99 {p99:.1} ms",
        window.resolution.physical_width(),
        window.resolution.physical_height(),
        triangles.get(),
    );
    if cave.enabled {
        let _ = write!(line, "   cave {} sections", cave.reached());
        if let Some(ms) = cave.took_ms() {
            let _ = write!(line, " in {ms:.3} ms");
        }
    } else {
        line.push_str("   cave off");
    }
    let status = loader.status();
    let _ = write!(
        line,
        "   arena {:.0}/{:.0}%",
        status.quads * 100.0,
        status.models * 100.0,
    );
    let _ = write!(
        line,
        "   {}/{} regions",
        status.regions, status.regions_total,
    );
    if status.files < status.files_total {
        let _ = write!(line, "   loading {}/{} files", status.files, status.files_total);
    }
    if status.evicted > 0 {
        let _ = write!(line, "   {} evicted", status.evicted);
    }
    for (stream, name) in mesh::STREAM_NAMES.iter().enumerate() {
        if streams.0 & (1 << stream) == 0 {
            let _ = write!(line, "   no {name}");
        }
    }
    for (slot, name) in probe::NAMES.iter().enumerate() {
        if let Some(ms) = gpu.median(slot) {
            let _ = write!(line, "   {name} {ms:.2} ms");
        }
    }
    let (hour, minute) = day.clock();
    let _ = write!(line, "   {hour:02}:{minute:02}");
    info!("{}", line);
    if let Some(mut overlay) = overlay {
        overlay.0.clear();
        overlay.0.push_str(line);
    }

    stats.frames = 0;
    stats.written = 0;
    stats.elapsed = 0.0;
}

fn toggle_wireframe(keys: Res<ButtonInput<KeyCode>>, mut wireframe: ResMut<Wireframe>) {
    if keys.just_pressed(KeyCode::F10) {
        wireframe.0 = !wireframe.0;
    }
}

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

fn screenshot(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    loader: Res<stream::Loader>,
    time: Res<Time>,
    mut settled: Local<u32>,
) {
    if loader.done() {
        *settled += 1;
    }
    let auto = std::env::var("ANVIL_SCREENSHOT").ok();
    let elapsed = time.elapsed_secs();
    let deadline = *settled == 30 || (elapsed >= 10.0 && elapsed - time.delta_secs() < 10.0);
    let path = match (&auto, keys.just_pressed(KeyCode::F12)) {
        (Some(path), _) if deadline => path.clone(),
        (_, true) => "anvil_region_viewer.png".to_string(),
        _ => {
            if auto.is_some() && elapsed > 60.0 {
                commands.write_message(AppExit::Success);
            }
            return;
        }
    };
    commands.spawn(Screenshot::primary_window()).observe(save_to_disk(path));
}

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
        Tonemapping::None,
        Msaa::Off,
        Projection::Perspective(PerspectiveProjection {
            far: 4000.0,
            ..default()
        }),
        Transform::default(),
        orbit,
    ));
}

fn spawn_overlay(mut commands: Commands) {
    if std::env::var("ANVIL_OVERLAY").is_ok_and(|on| on == "0") {
        return;
    }
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
