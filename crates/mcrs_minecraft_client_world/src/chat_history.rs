use bevy::prelude::*;
use bevy::tasks::{IoTaskPool, Task};
use std::path::PathBuf;

#[derive(Resource)]
pub struct CommandHistoryFile {
    path: PathBuf,
    writing: Option<Task<()>>,
    next: Option<String>,
}

impl CommandHistoryFile {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            writing: None,
            next: None,
        }
    }

    pub fn save(&mut self, file: String) {
        if self.writing.is_some() {
            self.next = Some(file);
        } else {
            self.writing = Some(write_history(self.path.clone(), file));
        }
    }
}

fn write_history(path: PathBuf, file: String) -> Task<()> {
    IoTaskPool::get().spawn(async move {
        let written = match path.parent() {
            Some(folder) => std::fs::create_dir_all(folder),
            None => Ok(()),
        }
        .and_then(|()| std::fs::write(&path, file));
        if let Err(error) = written {
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
        let folder =
            std::env::temp_dir().join(format!("mcrs-command-history-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&folder);
        let path = folder.join("command_history.txt");
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
        let _ = std::fs::remove_dir_all(&folder);
        assert_eq!(written.unwrap(), "/a\n/b\n/c\n");
    }
}
