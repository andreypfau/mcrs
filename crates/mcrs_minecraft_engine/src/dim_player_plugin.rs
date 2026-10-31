use crate::ability::{AnnounceOpLevel, AnswerOpLevel, OpLevelAnnounced, PlayerOpLevel};
use crate::client_info::{ClientInfo, ClientInfoPlugin};
use crate::column_view::ColumnViewPlugin;
use crate::consume_inbound_player_spawn::consume_inbound_player_spawn;
use crate::default_game_mode::DefaultGameMode;
use crate::despawn_inbound_player::despawn_inbound_player;
use crate::dimension_change::DimensionChanger;
use crate::game_mode::{self, GameModePlugin};
use crate::movement::MovementPlugin;
use crate::network_add::network_add;
use crate::op_level::{AnnouncedOpLevel, send_op_level};
use crate::persistence;
use crate::player_dimension_change::{self, CarryOnDimensionChangeExt};
use crate::session::GameProfile;
use bevy_app::{App, FixedUpdate, Plugin, Update};
use bevy_ecs::schedule::IntoScheduleConfigs;
use mcrs_minecraft_level::aoi::every_n_ticks;
use mcrs_minecraft_level::entity::player::chunk_view::PlayerViewDistance;
use mcrs_minecraft_level::tick::TickSet;

/// Plugin for per-dimension worlds. Registers only the bus-driven and
/// simulation systems that are safe to run inside a DimWorld — no
/// `ServerSideConnection` or other host-only resource is accessed.
pub struct DimPlayerPlugin;

impl Plugin for DimPlayerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DefaultGameMode>();
        app.insert_resource(DimensionChanger(player_dimension_change::change_dimension));
        app.add_plugins((MovementPlugin, ColumnViewPlugin, GameModePlugin));
        app.add_message::<OpLevelAnnounced>();
        app.configure_sets(Update, (AnnounceOpLevel, AnswerOpLevel).chain());
        if !app.is_plugin_added::<ClientInfoPlugin>() {
            app.add_plugins(ClientInfoPlugin);
        }
        app.carry_on_dimension_change::<GameProfile>()
            .carry_on_dimension_change::<PlayerViewDistance>()
            .carry_on_dimension_change::<PlayerOpLevel>()
            .carry_on_dimension_change::<ClientInfo>()
            .carry_on_dimension_change::<AnnouncedOpLevel>()
            .carry_on_dimension_change::<persistence::LoadedPlayerDat>()
            .carry_on_dimension_change::<persistence::UnreadablePlayerDat>();
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
                despawn_inbound_player.after(consume_inbound_player_spawn),
            ),
        );
        app.add_systems(
            FixedUpdate,
            persistence::autosave_players
                .in_set(TickSet::Output)
                .run_if(every_n_ticks(persistence::AUTOSAVE_INTERVAL)),
        );
        app.add_observer(network_add);
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
