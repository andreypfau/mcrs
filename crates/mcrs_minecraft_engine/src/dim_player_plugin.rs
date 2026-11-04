use crate::ability::{AnnounceOpLevel, AnswerOpLevel, OpLevelAnnounced};
use crate::client_info::ClientInfoPlugin;
use crate::column_streams::ColumnStreamPlugin;
use crate::consume_inbound_player_spawn::consume_inbound_player_spawn;
use crate::coordinator::default_game_mode::DefaultGameMode;
use crate::departure::PlayerTransfer;
use crate::despawn_inbound_player::despawn_inbound_player;
use crate::dim_tick::DimInboxDrain;
use crate::dim_tick::drain_dim_inbox;
use crate::game_mode::{self, GameModePlugin};
use crate::movement::MovementPlugin;
use crate::op_level::send_op_level;
use crate::persistence;
use crate::player_dimension_change::receive_transfers;
use crate::player_order::{OrderedPlayer, obey_orders};
use bevy_app::{App, FixedPreUpdate, FixedUpdate, Plugin, Update};
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_ecs::schedule::common_conditions::on_message;
use mcrs_minecraft_engine_world::aoi::every_n_ticks;
use mcrs_minecraft_engine_world::entity::add_entity_to_chunk;
use mcrs_minecraft_engine_world::entity::player::ChangeDimensionRequest;
use mcrs_minecraft_engine_world::tick::TickSet;

/// Plugin for per-dimension worlds. Registers only the bus-driven and
/// simulation systems that are safe to run inside a DimWorld — no
/// `ServerSideConnection` or other host-only resource is accessed.
pub struct DimPlayerPlugin;

impl Plugin for DimPlayerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DefaultGameMode>();
        app.init_resource::<mcrs_minecraft_engine_world::session::DimPlayerIndex>();
        app.add_message::<ChangeDimensionRequest>();
        app.add_message::<PlayerTransfer>();
        app.add_plugins((MovementPlugin, ColumnStreamPlugin, GameModePlugin));
        app.add_message::<OpLevelAnnounced>();
        app.configure_sets(Update, (AnnounceOpLevel, AnswerOpLevel).chain());
        if !app.is_plugin_added::<ClientInfoPlugin>() {
            app.add_plugins(ClientInfoPlugin);
        }
        app.add_systems(
            FixedPreUpdate,
            receive_transfers
                .run_if(on_message::<PlayerTransfer>)
                .in_set(DimInboxDrain)
                .after(drain_dim_inbox)
                .before(add_entity_to_chunk),
        );
        app.add_message::<OrderedPlayer>();
        app.add_systems(
            FixedPreUpdate,
            obey_orders
                .run_if(on_message::<OrderedPlayer>)
                .in_set(DimInboxDrain)
                .after(receive_transfers)
                .before(add_entity_to_chunk),
        );
        app.add_systems(
            Update,
            (
                (
                    (consume_inbound_player_spawn, send_op_level)
                        .chain()
                        .in_set(AnnounceOpLevel),
                    game_mode::send_game_mode.after(AnswerOpLevel),
                )
                    .chain(),
                // A player spawned and despawned in one drain must be saved and removed, or it
                // lingers and its autosaves overwrite the next session's file.
                despawn_inbound_player
                    .after(consume_inbound_player_spawn)
                    .after(game_mode::send_game_mode),
            ),
        );
        app.add_systems(
            FixedUpdate,
            persistence::autosave_players
                .in_set(TickSet::Output)
                .run_if(every_n_ticks(persistence::AUTOSAVE_INTERVAL)),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::schedule::ScheduleBuildError;

    #[test]
    fn a_despawn_is_handled_after_a_spawn_drained_in_the_same_tick() {
        let mut app = App::new();
        app.add_plugins(DimPlayerPlugin);
        // This closes a cycle exactly when the spawn is ordered before the despawn.
        app.add_systems(
            Update,
            (|| {})
                .after(despawn_inbound_player)
                .before(consume_inbound_player_spawn),
        );
        let built = app.world_mut().schedule_scope(Update, |world, schedule| {
            schedule.initialize(world).map(|_| ())
        });
        assert!(
            matches!(
                built,
                Err(ScheduleBuildError::CrossDependency(_) | ScheduleBuildError::DependencySort(_))
            ),
            "{built:?}"
        );
    }
}
