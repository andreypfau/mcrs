use bevy_ecs::observer::On;
use bevy_ecs::system::Commands;
use bevy_ecs::system::ResMut;

use crate::click_drag::ClickDrag;
use crate::creative_drops::CreativeDrops;
use crate::full_sends::FullSends;
use crate::last_container_id::LastContainerId;
use mcrs_minecraft_player_holders::PlayerItemsRestored;

pub(crate) fn queue_full_send(
    restored: On<PlayerItemsRestored>,
    mut sends: ResMut<FullSends>,
    mut commands: Commands,
) {
    let player = restored.entity;
    commands.entity(player).insert_if_new((
        ClickDrag::default(),
        CreativeDrops::default(),
        LastContainerId::default(),
    ));
    if !sends.0.contains(&player) {
        sends.0.push(player);
    }
}
