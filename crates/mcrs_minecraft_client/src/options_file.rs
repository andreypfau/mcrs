use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};

use bevy::prelude::*;
use bevy::tasks::futures;
use bevy::tasks::{IoTaskPool, Task, block_on};
use mcrs_minecraft_client_options::ClientOptions;

#[derive(Resource)]
struct OptionsFile {
    path: PathBuf,
    write: Option<Task<io::Result<()>>>,
    owed: bool,
    failed: bool,
}

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

fn owe_save(file: ResMut<OptionsFile>) {
    file.map_unchanged(|file| &mut file.owed).set_if_neq(true);
}

fn collect_write(mut file: ResMut<OptionsFile>) {
    let file = file.bypass_change_detection();
    if let Some(write) = &mut file.write
        && let Some(saved) = futures::check_ready(write)
    {
        file.failed = saved.is_err();
        file.write = None;
    }
}

fn save_pending(file: Res<OptionsFile>, buttons: Option<Res<ButtonInput<MouseButton>>>) -> bool {
    file.owed
        && file.write.is_none()
        && !buttons.is_some_and(|buttons| buttons.pressed(MouseButton::Left))
}

fn save_options(options: Res<ClientOptions>, mut file: ResMut<OptionsFile>) {
    let file = file.bypass_change_detection();
    file.owed = false;
    let Some(text) = options_text(&options) else {
        return;
    };
    let path = file.path.clone();
    file.write = Some(IoTaskPool::get().spawn(async move { write_options(&path, &text) }));
}

#[cfg_attr(
    mcrs_lint,
    expect(
        mcrs::blocking_in_system,
        reason = "the app exits after this frame, so the options must reach the disk before the process ends"
    )
)]
fn save_options_at_exit(options: Res<ClientOptions>, mut file: ResMut<OptionsFile>) {
    let file = file.bypass_change_detection();
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

fn replace_file(path: &Path, text: &str) -> io::Result<()> {
    use io::Write as _;

    if let Some(folder) = path.parent() {
        std::fs::create_dir_all(folder)?;
    }
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

        app.world_mut()
            .resource_mut::<ClientOptions>()
            .map_unchanged(|options| &mut options.fov)
            .set_if_neq(90);
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
        app.world_mut()
            .resource_mut::<ClientOptions>()
            .map_unchanged(|options| &mut options.fov)
            .set_if_neq(90);
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
        std::fs::write(&folder, "").unwrap();
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

        app.world_mut()
            .resource_mut::<ClientOptions>()
            .map_unchanged(|options| &mut options.fov)
            .set_if_neq(90);
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        while app.world().resource::<OptionsFile>().write.is_some() {
            app.update();
        }
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .release(MouseButton::Left);
        std::fs::remove_file(&folder).unwrap();
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
