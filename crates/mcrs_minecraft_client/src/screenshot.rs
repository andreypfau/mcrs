use std::fmt::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;

use bevy::asset::RenderAssetUsages;
use bevy::clipboard::Clipboard;
use bevy::input::common_conditions::input_just_pressed;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk};
use bevy::time::common_conditions::on_timer;

use mcrs_minecraft_render_probe::probe::{self, CpuTimings, GpuTimings};

use crate::Unsettled;
use crate::settle::Settle;

const DEFAULT_DIR: &str = "screenshots";

const TRIGGER: &str = "capture";

#[derive(Resource)]
pub struct ScreenshotDir(PathBuf);

impl ScreenshotDir {
    fn trigger(&self) -> PathBuf {
        self.0.join(TRIGGER)
    }

    fn next_shot(&self) -> PathBuf {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        self.0.join(format!("{stamp}.png"))
    }
}

pub struct ScreenshotPlugin;

impl Plugin for ScreenshotPlugin {
    fn build(&self, app: &mut App) {
        let knobs = crate::config::Knobs::of(app);
        let dir = knobs
            .screenshot_dir()
            .unwrap_or_else(|| PathBuf::from(DEFAULT_DIR));
        app.insert_resource(ScreenshotDir(dir))
            .add_systems(Startup, prepare_dir)
            .add_systems(
                Update,
                capture.run_if(
                    on_timer(Duration::from_secs_f32(TRIGGER_POLL_SECONDS))
                        .or_else(input_just_pressed(KeyCode::F2)),
                ),
            );
        if let Some(path) = knobs.capture_path() {
            app.insert_resource(SceneCapture(path))
                .add_message::<Unsettled>()
                .add_systems(Update, capture_scene);
        }
    }
}

fn prepare_dir(dir: Res<ScreenshotDir>) {
    if let Err(err) = std::fs::create_dir_all(&dir.0) {
        error!("cannot create {}: {err}", dir.0.display());
        return;
    }
    remove_trigger(&dir.trigger());
    info!(dir = %dir.0.display(), trigger = %dir.trigger().display(), "screenshots");
}

const TRIGGER_POLL_SECONDS: f32 = 0.25;

#[cfg_attr(
    mcrs_lint,
    expect(
        mcrs::blocking_in_system,
        reason = "the file name of a screenshot carries the wall-clock time"
    )
)]
#[cfg_attr(
    mcrs_lint,
    expect(
        mcrs::alloc_in_tick,
        reason = "polled by timer; it builds the file name only on the frame that finds the trigger file"
    )
)]
#[cfg_attr(mcrs_lint, mcrs::alloc_sites(1))]
fn capture(mut commands: Commands, keys: Res<ButtonInput<KeyCode>>, dir: Res<ScreenshotDir>) {
    let trigger = dir.trigger();
    if trigger.exists() {
        remove_trigger(&trigger);
        let shot = dir.next_shot();
        info!(path = %shot.display(), "capturing screenshot");
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(shot));
    }
    if keys.just_pressed(KeyCode::F2) {
        info!("capturing screenshot to the clipboard");
        commands
            .spawn(Screenshot::primary_window())
            .observe(copy_to_clipboard);
    }
}

#[derive(Resource)]
pub struct SceneCapture(pub PathBuf);

#[derive(Resource)]
struct SceneCaptured;

const SETTLE_SECS: f32 = 3.0;

const SETTLE_TIMEOUT_SECS: f32 = 180.0;

const TIMING_FRAMES: usize = 240;

#[derive(Default)]
struct Settling {
    settle: Settle,
    timed: usize,
    requested: bool,
}

#[cfg_attr(
    mcrs_lint,
    expect(
        mcrs::ungated_system,
        reason = "watches the scene settle on each frame until the capture, and only when the capture knob is set"
    )
)]
#[cfg_attr(
    mcrs_lint,
    expect(
        mcrs::alloc_in_tick,
        mcrs::blocking_in_system,
        reason = "allocates and writes the timing table only on the timeout and on the one frame that captures the scene"
    )
)]
#[cfg_attr(mcrs_lint, mcrs::alloc_sites(3))]
fn capture_scene(
    mut commands: Commands,
    capture: Res<SceneCapture>,
    streaming: crate::stream::Streaming,
    time: Res<Time<Real>>,
    gpu: Res<GpuTimings>,
    cpu: Res<CpuTimings>,
    mut unsettled: MessageReader<Unsettled>,
    captured: Option<Res<SceneCaptured>>,
    mut settling: Local<Settling>,
) {
    if captured.is_some() {
        commands.write_message(AppExit::Success);
        return;
    }
    if settling.requested {
        return;
    }
    let now = time.elapsed_secs();
    if now > SETTLE_TIMEOUT_SECS {
        let blockers = settling.settle.blockers();
        error!(
            path = %capture.0.display(),
            blocking = ?blockers.names(),
            unsettled_messages = blockers.unsettled_messages,
            columns = blockers.columns,
            queued = blockers.queued,
            meshing = blockers.meshing,
            uploads_waiting = blockers.uploads_waiting,
            "the scene did not settle within 180 s"
        );
        commands.write_message(AppExit::error());
        return;
    }
    let quiet = settling
        .settle
        .observe(time.elapsed(), &streaming, &mut unsettled);
    let Some(quiet) = quiet else {
        settling.timed = 0;
        return;
    };
    if quiet.as_secs_f32() < SETTLE_SECS {
        return;
    }
    settling.timed += 1;
    if settling.timed < TIMING_FRAMES {
        return;
    }
    let timings = capture.0.with_extension("tsv");
    if let Err(err) = std::fs::write(&timings, timing_rows(&gpu, &cpu)) {
        error!("cannot write {}: {err}", timings.display());
    }
    info!(path = %capture.0.display(), "capturing the settled scene");
    commands
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(capture.0.clone()))
        .observe(|_: On<ScreenshotCaptured>, mut commands: Commands| {
            commands.insert_resource(SceneCaptured);
        });
    settling.requested = true;
}

fn timing_rows(gpu: &GpuTimings, cpu: &CpuTimings) -> String {
    let gpu_rows = (0..probe::SLOTS).filter_map(|slot| {
        let name = probe::slot_name(slot);
        Some(("gpu", name, gpu.median_and_p95(slot, TIMING_FRAMES)?))
    });
    let cpu_rows = probe::CPU_NAMES
        .iter()
        .enumerate()
        .filter_map(|(slot, name)| Some(("cpu", *name, cpu.median_and_p95(slot, TIMING_FRAMES)?)));
    let mut rows = String::new();
    for (kind, name, [median, p95]) in gpu_rows.chain(cpu_rows) {
        let _ = writeln!(rows, "{kind}\t{name}\t{median:.4}\t{p95:.4}");
    }
    rows
}

fn copy_to_clipboard(captured: On<ScreenshotCaptured>, mut clipboard: ResMut<Clipboard>) {
    let pasteable = match opaque_rgba8(&captured.image) {
        Ok(image) => image,
        Err(err) => {
            error!("cannot convert the screenshot for the clipboard: {err}");
            return;
        }
    };
    match clipboard.set_image(&pasteable) {
        Ok(()) => info!("screenshot copied to the clipboard"),
        Err(err) => error!("cannot copy the screenshot to the clipboard: {err}"),
    }
}

fn opaque_rgba8(image: &Image) -> Result<Image, bevy::image::IntoDynamicImageError> {
    let mut rgba = image.clone().try_into_dynamic()?.into_rgba8();
    for pixel in rgba.pixels_mut() {
        pixel[3] = u8::MAX;
    }
    let (width, height) = rgba.dimensions();
    Ok(Image::new(
        Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        rgba.into_raw(),
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    ))
}

fn remove_trigger(trigger: &Path) {
    match std::fs::remove_file(trigger) {
        Ok(()) => {}
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
        Err(err) => error!("cannot clear {}: {err}", trigger.display()),
    }
}
