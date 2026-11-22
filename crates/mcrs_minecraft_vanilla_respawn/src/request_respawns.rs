use bevy_ecs::message::{MessageReader, MessageWriter};
use bevy_ecs::query::With;
use bevy_ecs::system::{Commands, Query};
use mcrs_minecraft_damage::Vitals;
use mcrs_minecraft_engine_damage::RespawnRequest;
use mcrs_minecraft_engine_world::entity::player::Player;

use crate::pending_spawn_search::PendingSpawnSearch;
use crate::respawn_command::RespawnCommand;
use crate::spawn_site::{SpawnSearch, SpawnSite};

pub(crate) fn request_respawns(
    mut commands: MessageReader<RespawnCommand>,
    players: Query<&Vitals, With<Player>>,
    site: SpawnSite,
    mut requests: MessageWriter<RespawnRequest>,
    mut placements: Commands,
) {
    for command in commands.read() {
        let Ok(vitals) = players.get(command.player) else {
            continue;
        };
        if vitals.health > 0.0 {
            continue;
        }
        let Some(respawn) = site.world_spawn() else {
            tracing::error!(
                player = ?command.player,
                "a dead player does not respawn: no dimension of the world spawn runs"
            );
            continue;
        };
        if respawn.search == SpawnSearch::InTarget {
            placements
                .entity(command.player)
                .try_insert(PendingSpawnSearch);
        }
        requests.write(RespawnRequest {
            player: command.player,
            dimension: respawn.dimension,
            transform: respawn.transform,
        });
    }
}
