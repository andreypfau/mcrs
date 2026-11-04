use crate::bus::{InboundPlayerDespawn, OutboundPlayerPacket, OutboundPlayerReleased};
use crate::entity_tracker::EntityTracker;
use crate::join_message;
use crate::persistence;
use crate::session::GameProfile;
use bevy_ecs::message::{MessageReader, MessageWriter};
use bevy_ecs::prelude::{Commands, Query, Res, With};
use bevy_ecs::world::World;
use mcrs_minecraft_engine_world::entity::player::Player;
use mcrs_minecraft_engine_world::session::DimPlayerIndex;

/// Per-dim consumer that despawns the in-dim player entity when an
/// `InboundPlayerDespawn` arrives for its session on a disconnect, so the
/// dimension stops streaming chunks toward that connection. The players near it hear that it
/// left.
pub fn despawn_inbound_player(
    mut reader: MessageReader<InboundPlayerDespawn>,
    index: Res<DimPlayerIndex>,
    players: Query<Option<&GameProfile>, With<Player>>,
    tracker: Option<Res<EntityTracker>>,
    mut writer: MessageWriter<OutboundPlayerPacket>,
    mut commands: Commands,
) {
    for msg in reader.read() {
        if let Some(&entity) = index.0.get(&msg.session)
            && let Ok(profile) = players.get(entity)
        {
            if let (Some(profile), Some(tracker)) = (profile, tracker.as_deref())
                && let Some(packet) = join_message::leave_message(tracker, entity, profile)
            {
                writer.write(packet);
            }
            commands.queue(move |world: &mut World| {
                persistence::write_player(world, entity);
                world.despawn(entity);
            });
        }
        // Queued after the save so the host never learns of the release before the file is written.
        let session = msg.session;
        commands.queue(move |world: &mut World| {
            world.write_message(OutboundPlayerReleased { session });
        });
    }
}
