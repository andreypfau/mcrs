use crate::bus::{InboundPlayerDespawn, OutboundPlayerReleased};
use crate::game_profile::GameProfile;
use crate::persisted_registry::CaptureCause;
use crate::persistence;
use crate::publish_player_tiles::publish_leave_message;
use crate::replicated::Replicated;
use crate::tile_events::TileEvents;
use bevy_ecs::message::MessageReader;
use bevy_ecs::prelude::{Commands, Query, Res, ResMut, With};
use bevy_ecs::schedule::SystemSet;
use bevy_ecs::world::World;
use mcrs_minecraft_engine_world::entity::player::Player;
use mcrs_minecraft_engine_world::session::DimPlayerIndex;

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InboundPlayerDespawnSet {
    Drop,
    Close,
}

pub fn despawn_inbound_player(
    mut reader: MessageReader<InboundPlayerDespawn>,
    index: Res<DimPlayerIndex>,
    players: Query<(Option<&GameProfile>, Option<&Replicated>), With<Player>>,
    mut events: Option<ResMut<TileEvents>>,
    mut commands: Commands,
) {
    for msg in reader.read() {
        if let Some(&entity) = index.0.get(&msg.session)
            && let Ok((profile, replicated)) = players.get(entity)
        {
            if let (Some(profile), Some(replicated), Some(events)) =
                (profile, replicated, events.as_deref_mut())
            {
                publish_leave_message(events, replicated, profile);
            }
            let session = msg.session;
            commands.queue(move |world: &mut World| {
                persistence::queue_player_save(world, entity, Some(session), CaptureCause::Leave);
                world.despawn(entity);
            });
        } else {
            let session = msg.session;
            commands.queue(move |world: &mut World| {
                world.write_message(OutboundPlayerReleased { session });
            });
        }
    }
}
