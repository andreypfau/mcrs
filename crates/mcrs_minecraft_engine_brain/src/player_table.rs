use bevy_ecs::prelude::With;
use bevy_ecs::system::Query;
use mcrs_minecraft_brain::PlayerFlags;
use mcrs_minecraft_brain::PlayerRow;
use mcrs_minecraft_brain::PlayerTable;
use mcrs_minecraft_engine::PlayerBody;
use mcrs_minecraft_engine::network_id::player_network_id;
use mcrs_minecraft_engine_world::entity::physics::Transform;
use mcrs_minecraft_engine_world::entity::player::Player;
use mcrs_minecraft_engine_world::session::Owner;
use mcrs_minecraft_entity_key::EntityId;
use mcrs_minecraft_entity_key::EntityUuid;
use mcrs_minecraft_game_mode::GameMode;
use mcrs_minecraft_game_mode::PlayerGameMode;

pub(crate) type Players<'w, 's> = Query<
    'w,
    's,
    (
        &'static Owner,
        &'static EntityUuid,
        &'static Transform,
        Option<&'static PlayerGameMode>,
        PlayerBody,
    ),
    With<Player>,
>;

pub(crate) fn player_table(players: &Players<'_, '_>) -> PlayerTable {
    PlayerTable::new(players.iter().map(|(owner, uuid, transform, mode, body)| {
        let spectator = mode.is_some_and(|mode| mode.0 == GameMode::Spectator);
        PlayerRow {
            player: EntityId(u64::from(player_network_id(owner.0).cast_unsigned())),
            key: *uuid,
            position: transform.translation,
            eye_height: body.dimensions().eye_height,
            flags: if spectator {
                PlayerFlags::ALIVE.union(PlayerFlags::SPECTATOR)
            } else {
                PlayerFlags::ALIVE
            },
            visibility: 1.0,
            vehicle: None,
        }
    }))
}
