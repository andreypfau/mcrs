use bevy::prelude::*;
use mcrs_minecraft_client_world::world_list::{PlayWorld, ReadWorldList, WorldList};

#[derive(Resource)]
pub struct PlayAtStart {
    pub folder_id: String,
}

pub fn read_world_list(mut reads: MessageWriter<ReadWorldList>) {
    reads.write(ReadWorldList);
}

pub fn play_at_start(
    play: Res<PlayAtStart>,
    list: Res<WorldList>,
    mut plays: MessageWriter<PlayWorld>,
    mut commands: Commands,
) {
    match list.as_ref() {
        WorldList::Reading => return,
        WorldList::Ready(worlds) => {
            if worlds.iter().any(|world| world.folder_id == play.folder_id) {
                plays.write(PlayWorld {
                    folder_id: play.folder_id.clone(),
                });
            } else {
                warn!("no world {} in the world list to play", play.folder_id);
            }
        }
        WorldList::Unavailable(error) => {
            warn!("no world list to play {} from: {error}", play.folder_id);
        }
    }
    commands.remove_resource::<PlayAtStart>();
}
