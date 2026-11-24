use bevy_app::App;
use bevy_app::Last;
use bevy_app::Plugin;
use bevy_app::Update;
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_ecs::schedule::SystemCondition;
use bevy_ecs::schedule::common_conditions::on_message;
use mcrs_minecraft_engine::bus::InboundPlayerDespawn;
use mcrs_minecraft_engine::despawn_inbound_player::InboundPlayerDespawnSet;
use mcrs_minecraft_engine::persisted_registry::PersistScope;
use mcrs_minecraft_engine::persisted_registry::PersistedAppExt;
use mcrs_minecraft_engine_world::entity::player::ChangeDimensionRequest;

use crate::carried_codec::CARRIED_KEYS;
use crate::carried_codec::apply_carried;
use crate::carried_codec::capture_carried;
use crate::departure_close::close_at_dimension_change;
use crate::departure_close::close_at_leave;
use crate::departure_close::departures_requested;
use crate::item_file_codec::register_ender_items;
use crate::player_file::FILE_KEYS;
use crate::player_file_codec::apply_player_file;
use crate::player_file_codec::capture_player_file;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct InventoryPersistencePlugin;

impl Plugin for InventoryPersistencePlugin {
    fn build(&self, app: &mut App) {
        register_ender_items(app);
        app.register_persisted_codec(
            FILE_KEYS,
            PersistScope::Persisted,
            capture_player_file,
            apply_player_file,
        )
        .register_persisted_codec(
            CARRIED_KEYS,
            PersistScope::TravelOnly,
            capture_carried,
            apply_carried,
        )
        .add_message::<InboundPlayerDespawn>()
        .add_message::<ChangeDimensionRequest>()
        .add_systems(
            Update,
            close_at_leave
                .run_if(on_message::<InboundPlayerDespawn>)
                .in_set(InboundPlayerDespawnSet::Close),
        )
        .add_systems(
            Last,
            close_at_dimension_change
                .run_if(on_message::<ChangeDimensionRequest>.or_else(departures_requested)),
        );
    }
}
