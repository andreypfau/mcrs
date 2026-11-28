use crate::ability::{AnnounceOpLevel, AnswerOpLevel, OpLevelAnnounced};
use crate::block_break::BlockBreakPlugin;
use crate::bus::{InboundPlayerDespawn, InboundPlayerSpawn, OutboundPlayerReleased};
use crate::client_info::ClientInfoPlugin;
use crate::client_view_distance::on_client_view_distance;
use crate::cold_columns::SaveRequest;
use crate::column_streams::ColumnStreamPlugin;
use crate::consume_inbound_player_spawn::{InboundPlayerSpawnSet, consume_inbound_player_spawn};
use crate::coordinator::default_game_mode::DefaultGameMode;
use crate::coordinator::ops::OpList;
use crate::departure::PlayerTransfer;
use crate::despawn_inbound_player::{InboundPlayerDespawnSet, despawn_inbound_player};
use crate::dim_tick::DimInboxDrain;
use crate::dim_tick::drain_dim_inbox;
use crate::game_mode::{self, GameModePlugin};
use crate::loaded_client::LoadedClientPlugin;
use crate::movement::MovementPlugin;
use crate::op_level::{refresh_op_levels, send_op_level};
use crate::persistence;
use crate::player_dimension_change::receive_transfers;
use crate::player_order::{OrderedPlayer, obey_orders};
use crate::player_sprinting::{PlayerSprinting, handle_player_command};
use crate::pose_systems::PoseSystems;
use crate::publish_pose::{OwnPoseEntries, publish_pose};
use crate::send_own_attributes::{OwnAttributesSet, send_own_attributes};
use crate::shift_key::{ShiftKey, handle_interact, handle_player_input};
use crate::swing::PlayerSwingPlugin;
use bevy_app::{App, FixedPreUpdate, FixedUpdate, Plugin, Update};
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_ecs::schedule::common_conditions::{
    on_message, resource_exists, resource_exists_and_changed,
};
use mcrs_minecraft_engine_world::entity::player::ChangeDimensionRequest;
use mcrs_minecraft_engine_world::entity::player::Player;
use mcrs_minecraft_engine_world::tick::TickSet;
use mcrs_minecraft_pose::Pose;

pub struct DimPlayerPlugin;

impl Plugin for DimPlayerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DefaultGameMode>();
        if !app.is_plugin_added::<persistence::PlayerPersistencePlugin>() {
            app.add_plugins(persistence::PlayerPersistencePlugin);
        }
        app.init_resource::<persistence::PlayerSaveQueue>();
        app.init_resource::<mcrs_minecraft_engine_world::session::DimPlayerIndex>();
        app.add_message::<ChangeDimensionRequest>();
        app.add_message::<PlayerTransfer>();
        app.add_plugins((
            MovementPlugin,
            ColumnStreamPlugin,
            GameModePlugin,
            BlockBreakPlugin,
        ));
        app.add_message::<OpLevelAnnounced>();
        app.configure_sets(Update, (AnnounceOpLevel, AnswerOpLevel).chain());
        if !app.is_plugin_added::<ClientInfoPlugin>() {
            app.add_plugins(ClientInfoPlugin);
        }
        app.add_observer(on_client_view_distance);
        if let Err(error) = app.try_register_required_components::<Player, PlayerSprinting>() {
            tracing::error!("a player does not require its sprint state: {error}");
        }
        app.add_observer(handle_player_command);
        if let Err(error) = app.try_register_required_components::<Player, ShiftKey>() {
            tracing::error!("a player does not require its shift key state: {error}");
        }
        if let Err(error) = app.try_register_required_components::<Player, Pose>() {
            tracing::error!("a player does not require its pose: {error}");
        }
        if let Err(error) = app.try_register_required_components::<Player, OwnPoseEntries>() {
            tracing::error!("a player does not require its own pose entries: {error}");
        }
        app.add_plugins((LoadedClientPlugin, PlayerSwingPlugin));
        app.add_observer(handle_player_input);
        app.add_observer(handle_interact);
        app.configure_sets(
            FixedUpdate,
            (
                PoseSystems::Select.in_set(TickSet::Simulate),
                PoseSystems::Publish
                    .in_set(TickSet::Project)
                    .before(OwnAttributesSet),
            ),
        );
        app.add_systems(FixedUpdate, publish_pose.in_set(PoseSystems::Publish));
        app.add_systems(
            FixedPreUpdate,
            receive_transfers
                .run_if(on_message::<PlayerTransfer>)
                .in_set(DimInboxDrain)
                .after(drain_dim_inbox),
        );
        app.add_message::<OrderedPlayer>();
        app.add_systems(
            FixedPreUpdate,
            obey_orders
                .run_if(on_message::<OrderedPlayer>)
                .in_set(DimInboxDrain)
                .after(receive_transfers),
        );
        add_join_and_leave(app);
        app.add_systems(
            FixedUpdate,
            send_own_attributes
                .in_set(TickSet::Project)
                .in_set(OwnAttributesSet),
        );
        app.add_message::<OutboundPlayerReleased>();
        app.add_systems(
            FixedUpdate,
            (
                persistence::autosave_players.run_if(resource_exists::<SaveRequest>),
                persistence::finish_player_saves.run_if(persistence::player_saves_pending),
            )
                .chain()
                .in_set(TickSet::Output),
        );
    }
}

fn add_join_and_leave(app: &mut App) {
    app.add_systems(
        Update,
        (
            (
                (
                    consume_inbound_player_spawn
                        .run_if(on_message::<InboundPlayerSpawn>)
                        .in_set(InboundPlayerSpawnSet),
                    refresh_op_levels.run_if(resource_exists_and_changed::<OpList>),
                    send_op_level,
                )
                    .chain()
                    .in_set(AnnounceOpLevel),
                game_mode::send_game_mode.after(AnswerOpLevel),
            )
                .chain(),
            despawn_inbound_player
                .run_if(on_message::<InboundPlayerDespawn>)
                .after(consume_inbound_player_spawn)
                .after(game_mode::send_game_mode),
        ),
    );
    app.configure_sets(
        Update,
        (
            InboundPlayerDespawnSet::Drop,
            InboundPlayerDespawnSet::Close,
        )
            .chain()
            .after(consume_inbound_player_spawn)
            .before(despawn_inbound_player),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::schedule::ScheduleBuildError;

    #[test]
    fn a_despawn_is_handled_after_a_spawn_drained_in_the_same_tick() {
        let mut app = App::new();
        app.add_plugins(DimPlayerPlugin);
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
