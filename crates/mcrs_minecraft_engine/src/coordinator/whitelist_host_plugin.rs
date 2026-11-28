use bevy_app::{App, Plugin};
use bevy_ecs::prelude::{Commands, On, ResMut};

use crate::whitelist::{EditWhitelist, EnforceWhitelist, Whitelist};

pub struct WhitelistHostPlugin;

impl Plugin for WhitelistHostPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Whitelist>();
        app.add_observer(apply_whitelist_edit);
    }
}

fn apply_whitelist_edit(
    edit: On<EditWhitelist>,
    mut whitelist: ResMut<Whitelist>,
    mut commands: Commands,
) {
    if whitelist.apply(edit.event()) {
        whitelist.write_file();
    }
    if !matches!(*edit, EditWhitelist::Add(_)) && whitelist.enforces() {
        commands.trigger(EnforceWhitelist);
    }
}
