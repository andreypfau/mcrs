use bevy_app::{App, Plugin};
use bevy_ecs::prelude::{Commands, On, ResMut};

use crate::banlist::{BanList, EditBans, EnforceBans};

pub struct BanListHostPlugin;

impl Plugin for BanListHostPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<BanList>();
        app.add_observer(apply_ban_edit);
    }
}

fn apply_ban_edit(edit: On<EditBans>, mut bans: ResMut<BanList>, mut commands: Commands) {
    if bans.apply(edit.event()) {
        bans.write_file();
        if matches!(*edit, EditBans::Add(_)) {
            commands.trigger(EnforceBans);
        }
    }
}
