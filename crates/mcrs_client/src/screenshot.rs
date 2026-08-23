use std::path::{Path, PathBuf};

use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};

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
    let triggered = trigger.exists();
    if triggered {
        remove_trigger(&trigger);
    }
    if !triggered && !keys.just_pressed(KeyCode::F2) {
        return;
    }
    let shot = dir.next_shot();
    info!(path = %shot.display(), "capturing screenshot");
    commands
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(shot));
}

fn remove_trigger(trigger: &Path) {
    match std::fs::remove_file(trigger) {
        Ok(()) => {}
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
        Err(err) => error!("cannot clear {}: {err}", trigger.display()),
    }
}
