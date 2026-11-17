use bevy_ecs::entity::Entity;
use bevy_ecs::system::Commands;
use bevy_ecs::system::In;
use bevy_ecs::system::Query;
use mcrs_minecraft_engine_item::HolderCommandsExt;
use mcrs_minecraft_engine_item::Slots;
use mcrs_minecraft_player_holders::PlayerHolders;

pub(crate) fn clear_items(
    In(player): In<Entity>,
    players: Query<&PlayerHolders>,
    holder_slots: Query<&Slots>,
    mut commands: Commands,
) {
    let Ok(holders) = players.get(player) else {
        return;
    };
    for holder in [
        player,
        holders.cursor,
        holders.craft_grid,
        holders.craft_result,
    ] {
        if let Ok(slots) = holder_slots.get(holder) {
            commands
                .entity(holder)
                .load_inventory(vec![None; slots.len()]);
        }
    }
}
