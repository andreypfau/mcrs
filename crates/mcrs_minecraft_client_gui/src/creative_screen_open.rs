use bevy::prelude::*;
use mcrs_minecraft_inventory_client::CreativeScreen;

use crate::creative_screen_state::CreativeScreenState;
use crate::creative_tab_content::CreativeTabContent;

pub(crate) fn open_creative_screen(
    _: On<Add<CreativeScreen>>,
    content: Res<CreativeTabContent>,
    mut creative: ResMut<CreativeScreenState>,
) {
    creative.open(&content);
}
