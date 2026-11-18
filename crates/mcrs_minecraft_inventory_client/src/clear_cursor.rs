use bevy_ecs::system::Commands;
use bevy_ecs::system::Query;
use mcrs_minecraft_engine_item::Slots;
use mcrs_minecraft_player_holders::PlayerHolders;

pub(crate) fn clear_cursor(
    player_holders: &PlayerHolders,
    holders: &Query<&Slots>,
    commands: &mut Commands,
) {
    if let Some(stack) = (holders.get(player_holders.cursor).ok()).and_then(|slots| slots.get(0)) {
        commands.entity(stack).despawn();
    }
}
