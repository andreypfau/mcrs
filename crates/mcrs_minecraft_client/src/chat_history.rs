//! The command history file, written on the I/O task pool.

use bevy::prelude::*;
use bevy::tasks::{IoTaskPool, Task};
use std::path::PathBuf;

/// The command history file and its writes. One write runs at a time and only the newest
/// history waits for it, so an older history never overwrites a newer one.
#[derive(Resource)]
pub(crate) struct CommandHistoryFile {
    path: PathBuf,
    writing: Option<Task<()>>,
    next: Option<String>,
}

impl CommandHistoryFile {
    pub(crate) fn new(path: PathBuf) -> Self {
        Self {
            path,
            writing: None,
            next: None,
        }
    }

    pub(crate) fn save(&mut self, file: String) {
        if self.writing.is_some() {
            self.next = Some(file);
        } else {
            self.writing = Some(write_history(self.path.clone(), file));
        }
    }
}

fn write_history(path: PathBuf, file: String) -> Task<()> {
    IoTaskPool::get().spawn(async move {
        if let Err(error) = std::fs::write(&path, file) {
            warn!("cannot write {}: {error}", path.display());
        }
    })
}

pub(crate) fn finish_history_write(mut file: ResMut<CommandHistoryFile>) {
    let file = &mut *file;
    if file
        .writing
        .as_ref()
        .is_some_and(|task| !task.is_finished())
    {
        return;
    }
    file.writing = file
        .next
        .take()
        .map(|next| write_history(file.path.clone(), next));
}

pub(crate) fn history_writing(file: Option<Res<CommandHistoryFile>>) -> bool {
    file.is_some_and(|file| file.writing.is_some())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saves_during_a_write_leave_the_file_with_the_newest_history() {
        let path =
            std::env::temp_dir().join(format!("mcrs-command-history-{}.txt", std::process::id()));
        let mut app = App::new();
        app.add_plugins(bevy::app::TaskPoolPlugin::default())
            .insert_resource(CommandHistoryFile {
                path: path.clone(),
                writing: None,
                next: None,
            })
            .add_systems(Update, finish_history_write.run_if(history_writing));
        {
            let mut file = app.world_mut().resource_mut::<CommandHistoryFile>();
            for history in ["/a\n", "/a\n/b\n", "/a\n/b\n/c\n"] {
                file.save(history.to_owned());
            }
        }
        for _ in 0..10_000 {
            if app
                .world()
                .resource::<CommandHistoryFile>()
                .writing
                .is_none()
            {
                break;
            }
            app.update();
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        assert!(
            app.world()
                .resource::<CommandHistoryFile>()
                .writing
                .is_none()
        );
        let written = std::fs::read_to_string(&path);
        let _ = std::fs::remove_file(&path);
        assert_eq!(written.unwrap(), "/a\n/b\n/c\n");
    }
}
