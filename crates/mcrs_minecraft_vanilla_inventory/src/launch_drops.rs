use bevy_ecs::hierarchy::ChildOf;
use bevy_ecs::message::MessageReader;
use bevy_ecs::query::With;
use bevy_ecs::system::Commands;
use bevy_ecs::system::EntityCommands;
use bevy_ecs::system::Query;
use mcrs_minecraft_engine::game_profile::GameProfile;
use mcrs_minecraft_engine::player_dimensions;
use mcrs_minecraft_engine_item::RequestDrop;
use mcrs_minecraft_engine_world::entity::physics::Transform;
use mcrs_minecraft_entity_attribute::AttributeInstances;
use mcrs_minecraft_item_entity::Cause;
use mcrs_minecraft_pose::Pose;
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
    players: Query<
        (
            Option<&Transform>,
            Option<&Pose>,
            Option<&AttributeInstances>,
            Option<&GameProfile>,
        ),
        With<PlayerHolders>,
    >,
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
        let Ok((transform, pose, attributes, profile)) = players.get(player) else {
            continue;
        };
        let eye_height = eye_height_of(pose, attributes);
        launch_from_player(
            &mut commands.entity(drop.stack),
            transform,
            eye_height,
            profile,
            cause,
        );
    }
}

pub(crate) fn eye_height_of(pose: Option<&Pose>, attributes: Option<&AttributeInstances>) -> f32 {
    player_dimensions(pose.copied().unwrap_or_default(), attributes).eye_height
}

pub(crate) fn launch_from_player(
    stack: &mut EntityCommands,
    transform: Option<&Transform>,
    eye_height: f32,
    profile: Option<&GameProfile>,
    cause: Cause,
) {
    let transform = transform.copied().unwrap_or(Transform::IDENTITY);
    let source = profile.map_or(0, |profile| profile.id.as_u128());
    stack.launch(hand_launch(transform, eye_height, source, cause));
    if cause == Cause::Throw
        && let Some(profile) = profile
    {
        stack.insert(Thrower(profile.id.as_u128()));
    }
}
