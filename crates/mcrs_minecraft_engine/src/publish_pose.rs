use bevy_ecs::component::Component;
use bevy_ecs::message::MessageWriter;
use bevy_ecs::query::{Changed, Or};
use bevy_ecs::system::{Query, ResMut};
use mcrs_engine_muxer::EntityEvent;
use mcrs_minecraft_engine_world::session::Owner;
use mcrs_minecraft_pose::Pose;
use mcrs_minecraft_protocol::VarInt;
use mcrs_minecraft_protocol_game::clientbound::ClientboundSetEntityData;
use mcrs_minecraft_protocol_game::entity::Metadata;
use mcrs_minecraft_protocol_game::entity_data_index::{SHARED_FLAG_SHIFT, SHARED_FLAG_SPRINT};
use mcrs_minecraft_replication::{PlayerData, PlayerEntries};

use crate::bus::{OutboundPlayerPacket, send_own_entity_state};
use crate::network_id::player_network_id;
use crate::player_sprinting::PlayerSprinting;
use crate::replicated::Replicated;
use crate::shift_key::ShiftKey;
use crate::tile_events::TileEvents;

#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct OwnPoseEntries(PlayerEntries);

type PoseInputsChanged = Or<(Changed<Pose>, Changed<ShiftKey>, Changed<PlayerSprinting>)>;

pub(crate) fn publish_pose(
    mut players: Query<
        (
            &Owner,
            &Pose,
            &ShiftKey,
            &PlayerSprinting,
            &mut OwnPoseEntries,
            Option<&Replicated>,
        ),
        PoseInputsChanged,
    >,
    mut writer: MessageWriter<OutboundPlayerPacket>,
    mut events: Option<ResMut<TileEvents>>,
) {
    for (&Owner(session), &pose, &shift, &sprinting, mut own, replicated) in &mut players {
        let entries = pose_entries(pose, shift, sprinting);
        let changed = entries.changed_from(&own.0);
        if changed.is_empty() {
            continue;
        }
        if let (Some(events), Some(replicated)) = (events.as_deref_mut(), replicated) {
            events.push(
                replicated.tile,
                replicated.id,
                EntityEvent::Data(PlayerData::Entries(entries).into()),
            );
        }
        own.0 = entries;
        let packet = ClientboundSetEntityData {
            entity_id: VarInt(player_network_id(session)),
            metadata: Metadata(changed),
        };
        send_own_entity_state(&mut writer, session, &packet, "pose entries");
    }
}

#[must_use]
pub(crate) fn pose_entries(
    pose: Pose,
    shift: ShiftKey,
    sprinting: PlayerSprinting,
) -> PlayerEntries {
    let mut shared_flags = 0;
    if shift.0 {
        shared_flags |= SHARED_FLAG_SHIFT;
    }
    if sprinting.0 {
        shared_flags |= SHARED_FLAG_SPRINT;
    }
    PlayerEntries { shared_flags, pose }
}
