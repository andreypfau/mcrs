use bevy_ecs::hierarchy::ChildOf;
use bevy_ecs::message::MessageReader;
use bevy_ecs::query::With;
use bevy_ecs::system::Commands;
use bevy_ecs::system::EntityCommands;
use bevy_ecs::system::Query;
use mcrs_minecraft_engine::game_profile::GameProfile;
use mcrs_minecraft_engine_item::RequestDrop;
use mcrs_minecraft_engine_world::entity::physics::Transform;
use mcrs_minecraft_item_entity::Cause;
use mcrs_minecraft_vanilla_item_entity::ItemEntityCommandsExt;
use mcrs_minecraft_vanilla_item_entity::Thrower;

use crate::PlayerHolders;
use crate::hand_launch::hand_launch;
use crate::request_ids::CLICK_REQUEST;
use crate::request_ids::CLOSE_REQUEST;
use crate::request_ids::HAND_DROP_REQUEST;

pub(crate) fn launch_drops(
    mut drops: MessageReader<RequestDrop>,
    parents: Query<&ChildOf>,
    players: Query<(Option<&Transform>, Option<&GameProfile>), With<PlayerHolders>>,
    mut commands: Commands,
) {
    for drop in drops.read() {
        let cause = match drop.id {
            CLOSE_REQUEST => Cause::MenuRest,
            CLICK_REQUEST | HAND_DROP_REQUEST => Cause::Throw,
            _ => continue,
        };
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
        launch_from_player(&mut commands.entity(drop.stack), transform, profile, cause);
    }
}

pub(crate) fn launch_from_player(
    stack: &mut EntityCommands,
    transform: Option<&Transform>,
    profile: Option<&GameProfile>,
    cause: Cause,
) {
    let transform = transform.copied().unwrap_or(Transform::IDENTITY);
    let source = profile.map_or(0, |profile| profile.id.as_u128());
    stack.launch(hand_launch(transform, source, cause));
    if cause == Cause::Throw
        && let Some(profile) = profile
    {
        stack.insert(Thrower(profile.id.as_u128()));
    }
}
