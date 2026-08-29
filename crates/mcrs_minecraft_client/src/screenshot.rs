use std::path::{Path, PathBuf};

use bevy::asset::RenderAssetUsages;
use bevy::clipboard::Clipboard;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk};

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
