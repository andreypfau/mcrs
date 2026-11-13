use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};

use bevy::prelude::*;
use bevy::tasks::futures;
use bevy::tasks::{IoTaskPool, Task, block_on};
use mcrs_minecraft_client_options::ClientOptions;

/// The file the options are saved to. At most one write runs at a time, so a later value never
/// lands before an earlier one; a change during a write is owed and saved after it. A write that
/// failed is tried again when the client stops and not on each frame: its cause seldom passes
/// (MENU-DEV-005, MENU-DEV-006).
#[derive(Resource)]
struct OptionsFile {
    path: PathBuf,
    write: Option<Task<io::Result<()>>>,
    owed: bool,
    failed: bool,
}

/// Writes the options to `path` as JSON each time they change after start-up.
pub(crate) fn save_options_to(app: &mut App, path: PathBuf) {
    app.insert_resource(OptionsFile {
        path,
        write: None,
        owed: false,
        failed: false,
    })
    .add_systems(
        Last,
        (
            owe_save.run_if(
                resource_changed::<ClientOptions>.and_then(not(resource_added::<ClientOptions>)),
            ),
            collect_write.run_if(|file: Res<OptionsFile>| file.write.is_some()),
            save_options.run_if(save_pending),
            save_options_at_exit.run_if(on_message::<AppExit>),
        )
            .chain(),
    );
}

fn owe_save(mut file: ResMut<OptionsFile>) {
    file.owed = true;
}

/// Takes the outcome of a write that ended. The held left button does not delay it, so the stop
/// of the client knows of each failure that came before it.
fn collect_write(mut file: ResMut<OptionsFile>) {
    if let Some(write) = &mut file.write
        && let Some(saved) = futures::check_ready(write)
    {
        file.failed = saved.is_err();
        file.write = None;
    }
}

/// A slider drag holds the left button; its steps are saved once, after the release.
fn save_pending(file: Res<OptionsFile>, buttons: Option<Res<ButtonInput<MouseButton>>>) -> bool {
    file.owed
        && file.write.is_none()
        && !buttons.is_some_and(|buttons| buttons.pressed(MouseButton::Left))
}

fn save_options(options: Res<ClientOptions>, mut file: ResMut<OptionsFile>) {
    file.owed = false;
    let Some(text) = options_text(&options) else {
        return;
    };
    let path = file.path.clone();
    file.write = Some(IoTaskPool::get().spawn(async move { write_options(&path, &text) }));
}

fn save_options_at_exit(options: Res<ClientOptions>, mut file: ResMut<OptionsFile>) {
    // The client ends after this frame, so a write that still runs may block it. If it fails,
    // its cause has no time to pass before the end, so it is not tried again.
    if let Some(write) = file.write.take() {
        let _ = block_on(write);
        file.failed = false;
    }
    if (file.owed || file.failed)
        && let Some(text) = options_text(&options)
    {
        file.owed = false;
        file.failed = write_options(&file.path, &text).is_err();
    }
}

fn options_text(options: &ClientOptions) -> Option<String> {
    serde_json::to_string_pretty(options)
        .inspect_err(|error| warn!("the client options do not serialize: {error}"))
        .ok()
}

fn write_options(path: &Path, text: &str) -> io::Result<()> {
    replace_file(path, text).inspect_err(|error| {
        warn!(
            "the client options cannot be saved to {}: {error}",
            path.display()
        );
    })
}

/// Writes `text` to a file beside `path`, flushes it to the disk and renames it over `path`, so
/// a write that a crash or a power loss cuts short keeps the last saved options: the client
/// refuses to start on an options file it cannot read (MENU-DEV-007).
fn replace_file(path: &Path, text: &str) -> io::Result<()> {
    use io::Write as _;

    let mut partial = path.as_os_str().to_owned();
    partial.push(".tmp");
    let mut file = File::create(&partial)?;
    file.write_all(text.as_bytes())?;
    file.sync_all()?;
    std::fs::rename(&partial, path)
}

#[cfg(test)]
mod tests {
    use bevy::app::TaskPoolPlugin;

    use super::*;
    use crate::options::ClientOptionsPlugin;

    #[test]
    fn a_change_after_start_up_saves_the_options_once() {
        let path = std::env::temp_dir().join(format!("mcrs-options-{}.json", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let mut app = App::new();
        app.add_plugins((
            TaskPoolPlugin::default(),
            ClientOptionsPlugin {
                options: ClientOptions::default(),
                file: Some(path.clone()),
                gui_scale: None,
            },
        ));
        app.update();
        app.update();
        assert!(!path.exists(), "the start-up options are not saved");

        app.world_mut().resource_mut::<ClientOptions>().fov = 90;
        app.update();
        while app.world().resource::<OptionsFile>().write.is_some() {
            app.update();
        }
        let saved: ClientOptions =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(saved, *app.world().resource::<ClientOptions>());

        std::fs::remove_file(&path).unwrap();
        app.update();
        app.update();
        assert!(!path.exists(), "one change is saved once");
    }

    #[test]
    fn a_change_under_the_held_left_button_waits_and_the_exit_saves_it() {
        let path =
            std::env::temp_dir().join(format!("mcrs-options-held-{}.json", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let mut app = App::new();
        app.add_plugins((
            TaskPoolPlugin::default(),
            ClientOptionsPlugin {
                options: ClientOptions::default(),
                file: Some(path.clone()),
                gui_scale: None,
            },
        ))
        .init_resource::<ButtonInput<MouseButton>>();
        app.update();

        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.world_mut().resource_mut::<ClientOptions>().fov = 90;
        app.update();
        assert!(!path.exists(), "a drag is saved after its release");

        app.world_mut().write_message(AppExit::Success);
        app.update();
        let saved: ClientOptions =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(saved.fov, 90);
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn a_save_that_failed_is_tried_again_only_when_the_client_stops() {
        let folder =
            std::env::temp_dir().join(format!("mcrs-options-retry-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&folder);
        let path = folder.join("options.json");
        let mut app = App::new();
        app.add_plugins((
            TaskPoolPlugin::default(),
            ClientOptionsPlugin {
                options: ClientOptions::default(),
                file: Some(path.clone()),
                gui_scale: None,
            },
        ))
        .init_resource::<ButtonInput<MouseButton>>();
        app.update();

        app.world_mut().resource_mut::<ClientOptions>().fov = 90;
        app.update();
        // The held button must not hide the failure from the stop.
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        while app.world().resource::<OptionsFile>().write.is_some() {
            app.update();
        }
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .release(MouseButton::Left);
        std::fs::create_dir(&folder).unwrap();
        app.update();
        app.update();
        assert!(!path.exists(), "a later frame does not try the save again");

        app.world_mut().write_message(AppExit::Success);
        app.update();
        let saved: ClientOptions =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(saved.fov, 90);
        std::fs::remove_dir_all(&folder).unwrap();
    }
}
