use std::fmt::Write;
use std::path::{Path, PathBuf};

use bevy::asset::RenderAssetUsages;
use bevy::clipboard::Clipboard;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk};

use mcrs_minecraft_render::probe::{self, CpuTimings, GpuTimings};
use mcrs_minecraft_render::{RenderPath, ShownPath};
use mcrs_minecraft_render_deferred::VolumeQueue;

use crate::light_volume::LightVolumeFeed;

const DIR_VAR: &str = "MCRS_SCREENSHOT_DIR";

const DEFAULT_DIR: &str = "screenshots";

const TRIGGER: &str = "capture";

/// Where screenshots land, and where the trigger file is watched for.
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
        let dir = std::env::var_os(DIR_VAR)
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(DEFAULT_DIR));
        app.insert_resource(ScreenshotDir(dir))
            .add_systems(Startup, prepare_dir)
            .add_systems(Update, capture);
        if let Some(path) = crate::config::capture_path() {
            app.insert_resource(SceneCapture(path))
                .add_systems(Update, capture_scene);
        }
        #[cfg(feature = "telemetry-tracy")]
        if crate::config::tracy_preview() {
            app.init_resource::<tracy_preview::Preview>()
                .add_systems(Update, tracy_preview::request);
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

fn capture(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    dir: Res<ScreenshotDir>,
    time: Res<Time>,
    mut due: Local<f32>,
) {
    let trigger = dir.trigger();
    let now = time.elapsed_secs();
    let poll = now >= *due;
    if poll {
        *due = now + TRIGGER_POLL_SECONDS;
    }
    if poll && trigger.exists() {
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

/// Where a scripted run writes its one screenshot of the settled scene.
#[derive(Resource)]
pub struct SceneCapture(pub PathBuf);

#[derive(Resource)]
struct SceneCaptured;

const SETTLE_SECS: f32 = 3.0;

const SETTLE_TIMEOUT_SECS: f32 = 180.0;

/// Kept below the GPU probe's 256-sample window, so every GPU figure comes from these frames.
const TIMING_FRAMES: usize = 240;

#[derive(Default)]
struct Settling {
    quiet_since: Option<f32>,
    last: (usize, usize, usize),
    timed: usize,
    requested: bool,
}

/// `Streaming::done` also holds in the gaps between the server's batches, so the
/// scene counts as settled only once it has stayed done with nothing new arriving. On the
/// deferred path it also waits for block light colour, with no timeout of its own: colour that
/// never settles fails the capture rather than shooting it uncoloured.
fn capture_scene(
    mut commands: Commands,
    capture: Res<SceneCapture>,
    streaming: crate::stream::Streaming,
    time: Res<Time<Real>>,
    gpu: Res<GpuTimings>,
    cpu: Res<CpuTimings>,
    requested: Res<RenderPath>,
    shown: Res<ShownPath>,
    feed: Res<LightVolumeFeed>,
    volume: Res<VolumeQueue>,
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
        error!(path = %capture.0.display(), "the scene did not settle within 180 s");
        commands.write_message(AppExit::error());
        return;
    }
    let status = streaming.status();
    let counts = (status.columns, status.sections, status.sections_total);
    let coloured = shown.get() != RenderPath::Deferred || (feed.idle() && volume.idle());
    let quiet = streaming.done()
        && status.columns > 0
        && counts == settling.last
        && shown.get() == *requested
        && coloured;
    settling.last = counts;
    if !quiet {
        settling.quiet_since = None;
        settling.timed = 0;
        return;
    }
    let since = *settling.quiet_since.get_or_insert(now);
    if now - since < SETTLE_SECS {
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
    let path = capture.0.with_extension("path");
    if let Err(err) = std::fs::write(&path, format!("{}\n", shown.get().name())) {
        error!("cannot write {}: {err}", path.display());
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
    let gpu_rows = probe::NAMES
        .iter()
        .enumerate()
        .filter_map(|(slot, name)| Some(("gpu", name, gpu.median_and_p95(slot, TIMING_FRAMES)?)));
    let cpu_rows = probe::CPU_NAMES
        .iter()
        .enumerate()
        .filter_map(|(slot, name)| Some(("cpu", name, cpu.median_and_p95(slot, TIMING_FRAMES)?)));
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

/// The clipboard takes packed RGBA8 where the window hands over its own
/// swapchain format, and with HDR on the alpha channel carries brightness
/// rather than opacity, which would paste as a see-through image.
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

/// Tracy shows a frame preview only for frames the client sent an image for, so
/// the profiler has nothing to draw until the window is read back and handed to
/// it explicitly.
#[cfg(feature = "telemetry-tracy")]
mod tracy_preview {
    use super::*;

    /// Tracy wants both dimensions divisible by four.
    const WIDTH: u32 = 320;

    const HEIGHT: u32 = 180;

    /// A readback per frame at window resolution would dominate the very timings
    /// being profiled, so the next capture waits for the one in flight and the
    /// rate settles wherever the readback keeps up.
    #[derive(Resource, Default)]
    pub(super) struct Preview {
        frame: u32,
        in_flight: Option<u32>,
    }

    pub(super) fn request(mut commands: Commands, mut preview: ResMut<Preview>) {
        preview.frame = preview.frame.wrapping_add(1);
        if preview.in_flight.is_some() {
            return;
        }
        preview.in_flight = Some(preview.frame);
        commands.spawn(Screenshot::primary_window()).observe(send);
    }

    fn send(captured: On<ScreenshotCaptured>, mut preview: ResMut<Preview>) {
        let captured_at = preview.in_flight.take();
        let rgba = match captured.image.clone().try_into_dynamic() {
            Ok(image) => image.into_rgba8(),
            Err(err) => {
                error!("cannot convert the screenshot for tracy: {err}");
                return;
            }
        };
        let (width, height) = rgba.dimensions();
        if width == 0 || height == 0 {
            return;
        }
        let (out_width, out_height) = fit(width, height);
        let mut scaled = Vec::with_capacity((out_width * out_height * 4) as usize);
        for y in 0..out_height {
            let source_y = (y * height / out_height).min(height - 1);
            for x in 0..out_width {
                let source_x = (x * width / out_width).min(width - 1);
                let pixel = rgba.get_pixel(source_x, source_y).0;
                scaled.extend_from_slice(&[pixel[0], pixel[1], pixel[2], u8::MAX]);
            }
        }
        // Tracy pins the image back onto the frame that produced it, which is as
        // many frames ago as the readback took to land here.
        let delay = captured_at
            .map(|at| preview.frame.wrapping_sub(at))
            .unwrap_or(0);
        tracing_tracy::client::frame_image(
            &scaled,
            out_width as u16,
            out_height as u16,
            delay.try_into().unwrap_or(u8::MAX),
            false,
        );
    }

    /// The largest box within `WIDTH` by `HEIGHT` that keeps the window's aspect
    /// ratio, rounded down to what Tracy accepts.
    fn fit(width: u32, height: u32) -> (u32, u32) {
        let (mut out_width, mut out_height) = (width, height);
        if out_width > WIDTH {
            out_width = WIDTH;
            out_height = height * WIDTH / width;
        }
        if out_height > HEIGHT {
            out_width = width * HEIGHT / height;
            out_height = HEIGHT;
        }
        ((out_width & !3).max(4), (out_height & !3).max(4))
    }

    #[cfg(test)]
    mod tests {
        use super::fit;

        #[test]
        fn fit_stays_within_tracy_limits() {
            for (width, height) in [(2560, 1440), (1920, 1080), (1024, 768), (100, 3000), (7, 5)] {
                let (out_width, out_height) = fit(width, height);
                assert!(out_width <= super::WIDTH && out_height <= super::HEIGHT);
                assert_eq!((out_width % 4, out_height % 4), (0, 0));
            }
        }
    }
}
