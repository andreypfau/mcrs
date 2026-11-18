use bevy_ecs::change_detection::DetectChangesMut;
use bevy_ecs::entity::Entity;
use bevy_ecs::query::With;
use bevy_ecs::system::In;
use bevy_ecs::system::Query;
use mcrs_minecraft_engine_item::Selected;
use mcrs_minecraft_inventory_player::HOTBAR;
use mcrs_minecraft_player_holders::PlayerHolders;

pub(crate) fn apply_selected_slot(
    In((player, slot)): In<(Entity, u8)>,
    mut players: Query<&mut Selected, With<PlayerHolders>>,
) {
    if !HOTBAR.contains(&u16::from(slot)) {
        return;
    }
    if let Ok(mut selected) = players.get_mut(player) {
        selected.set_if_neq(Selected(slot));
    }
}
