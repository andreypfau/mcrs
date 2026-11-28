use bevy::prelude::*;
use bevy::time::Real;
use mcrs_minecraft_inventory_client::CreativeScreen;

use crate::creative_screen_state::CreativeScreenState;
use crate::creative_tab_content::CreativeTabContent;
use crate::menu_input;

pub(crate) fn open_creative_screen(
    _: On<Add<CreativeScreen>>,
    content: Res<CreativeTabContent>,
    time: Option<Res<Time<Real>>>,
    mut creative: ResMut<CreativeScreenState>,
) {
    creative.open(&content);
    if let Some(time) = time {
        creative.focus_search(menu_input::menu_clock(&time));
    }
}
