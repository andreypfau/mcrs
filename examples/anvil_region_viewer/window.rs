use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use bevy::window::{Monitor, MonitorSelection, WindowMode, WindowPosition};

use crate::{config, stream};

pub fn title(path: &str) -> String {
    let name = std::path::Path::new(path)
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string());
    format!("anvil region viewer — {name}")
}

pub fn starting_mode() -> WindowMode {
    match config::fullscreen_mode(MonitorSelection::Current) {
        Some(_) => WindowMode::BorderlessFullscreen(MonitorSelection::Current),
        None => WindowMode::Windowed,
    }
}

pub fn place(
    window: Single<&mut Window>,
    monitors: Query<(Entity, &Monitor)>,
    mut placed: Local<bool>,
) {
    if *placed || monitors.is_empty() {
        return;
    }
    let monitor = chosen_monitor(&monitors);
    let mut window = window;
    match config::fullscreen_mode(monitor) {
        Some(mode) => window.mode = mode,
        None => window.position = WindowPosition::Centered(monitor),
    }
    *placed = true;
}

fn chosen_monitor(monitors: &Query<(Entity, &Monitor)>) -> MonitorSelection {
    let Some(spec) = config::monitor_spec() else {
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

pub fn toggle_fullscreen(keys: Res<ButtonInput<KeyCode>>, window: Single<&mut Window>) {
    if !keys.just_pressed(KeyCode::F11) {
        return;
    }
    let mut window = window;
    window.mode = match window.mode {
        WindowMode::Windowed => WindowMode::BorderlessFullscreen(MonitorSelection::Current),
        _ => WindowMode::Windowed,
    };
}

pub fn screenshot(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    loader: Res<stream::Loader>,
    time: Res<Time>,
    mut settled: Local<u32>,
) {
    if loader.done() {
        *settled += 1;
    }
    let auto = config::screenshot_path();
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
