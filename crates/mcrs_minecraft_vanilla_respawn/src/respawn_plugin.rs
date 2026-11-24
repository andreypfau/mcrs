use bevy_app::{App, FixedUpdate, Plugin, Update};
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_ecs::schedule::common_conditions::{any_with_component, on_message};
use mcrs_minecraft_engine::block_simulation::BlockSimulationSet;
use mcrs_minecraft_engine::persisted_registry::{PersistScope, PersistedAppExt};
use mcrs_minecraft_engine::{HeldJoins, InboundPlayerSpawnSet};
use mcrs_minecraft_engine_damage::{DamageSet, RespawnRequest};
use mcrs_minecraft_engine_world::tick::TickSet;

use crate::pending_spawn_search::{
    PENDING_SPAWN_SEARCH_KEYS, PendingSpawnSearch, apply_pending_spawn_search,
    capture_pending_spawn_search, finish_spawn_searches,
};
use crate::place_held_joins::{holds_joins, place_held_joins};
use crate::request_respawns::request_respawns;
use crate::respawn_command::{RespawnCommand, on_client_command};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RespawnPlugin;

impl Plugin for RespawnPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<HeldJoins>()
            .add_message::<RespawnCommand>()
            .add_message::<RespawnRequest>()
            .register_persisted_codec(
                PENDING_SPAWN_SEARCH_KEYS,
                PersistScope::TravelOnly,
                capture_pending_spawn_search,
                apply_pending_spawn_search,
            )
            .add_observer(on_client_command)
            .add_systems(
                Update,
                place_held_joins
                    .run_if(holds_joins)
                    .before(InboundPlayerSpawnSet),
            )
            .add_systems(
                FixedUpdate,
                (
                    finish_spawn_searches.run_if(any_with_component::<PendingSpawnSearch>),
                    request_respawns.run_if(on_message::<RespawnCommand>),
                )
                    .chain()
                    .in_set(TickSet::WorldWrite)
                    .after(BlockSimulationSet)
                    .before(DamageSet::Collect),
            );
    }
}
