use bevy_ecs::hierarchy::ChildOf;
use bevy_ecs::message::MessageReader;
use bevy_ecs::query::With;
use bevy_ecs::system::Commands;
use bevy_ecs::system::Query;
use mcrs_minecraft_engine::game_profile::GameProfile;
use mcrs_minecraft_engine_item::RequestDrop;
use mcrs_minecraft_engine_world::entity::physics::Transform;
use mcrs_minecraft_item_entity::Cause;
use mcrs_minecraft_vanilla_item_entity::ItemEntityCommandsExt;
use mcrs_minecraft_vanilla_item_entity::Thrower;

use crate::PlayerHolders;
use crate::hand_launch::hand_launch;

/// Throws each stack that a drop took out of a holder of a player as an item entity with the
/// player as its thrower: a throw click, a click outside the menu or a drop key (IEN-SPN-001).
pub(crate) fn launch_drops(
    mut drops: MessageReader<RequestDrop>,
    parents: Query<&ChildOf>,
    players: Query<(Option<&Transform>, Option<&GameProfile>), With<PlayerHolders>>,
    mut commands: Commands,
) {
    for drop in drops.read() {
        let player = if players.contains(drop.holder) {
            drop.holder
        } else if let Ok(parent) = parents.get(drop.holder) {
            parent.parent()
        } else {
            continue;
        };
        let Ok((transform, profile)) = players.get(player) else {
            continue;
        };
        let transform = transform.copied().unwrap_or(Transform::IDENTITY);
        let thrower = profile.map(|profile| Thrower(profile.id.as_u128()));
        let source = thrower.map_or(0, |thrower| thrower.0);
        let mut stack = commands.entity(drop.stack);
        stack.launch(hand_launch(transform, source, Cause::Throw));
        if let Some(thrower) = thrower {
            stack.insert(thrower);
        }
    }
}
