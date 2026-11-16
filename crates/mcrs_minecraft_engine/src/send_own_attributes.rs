use bevy_ecs::change_detection::{DetectChanges, DetectChangesMut};
use bevy_ecs::entity::Entity;
use bevy_ecs::message::{MessageReader, MessageWriter};
use bevy_ecs::query::Changed;
use bevy_ecs::schedule::SystemSet;
use bevy_ecs::system::{Local, Query, ResMut};
use bytes::{BufMut, BytesMut};
use mcrs_engine_muxer::EntityEvent;
use mcrs_minecraft_engine_world::session::Owner;
use mcrs_minecraft_entity_attribute::attribute_keys::Attribute;
use mcrs_minecraft_entity_attribute::{AttributeInstances, AttributeSnapshot};
use mcrs_minecraft_protocol::{Packet, VarInt};
use mcrs_minecraft_protocol_game::clientbound::ClientboundUpdateAttributes;
use mcrs_minecraft_replication::{AttributeState, PlayerData};

use crate::ability::OpLevelAnnounced;
use crate::bus::{OutboundPlayerPacket, PacketPayload, to};
use crate::network_id::player_network_id;
use crate::replicated::Replicated;
use crate::tile_events::TileEvents;

/// The system that sends each player its own attribute instances and records the changed
/// instances of a published player in the tile events.
#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct OwnAttributesSet;

/// Gives the client of each player one `update_attributes` about its own entity, inside the
/// probes of its prediction (MVC-PKT-031). It holds the state of each syncable instance that
/// changed since the last run (ATR-NET-003), and, for a player that a `login` or a `respawn` just
/// placed, each stored syncable instance (ATR-NET-005). It also records the state of each changed
/// instance of a published player for the players that track it (ATR-NET-003). It is the one
/// reader of the changed attributes of a player.
// A placed player is a new entity whose operator level is announced with the `login` or the
// `respawn`, which went out before the tick that runs this system.
pub(crate) fn send_own_attributes(
    mut announced: MessageReader<OpLevelAnnounced>,
    mut players: Query<
        (Entity, &mut AttributeInstances, &Owner, Option<&Replicated>),
        Changed<AttributeInstances>,
    >,
    mut writer: MessageWriter<OutboundPlayerPacket>,
    mut events: Option<ResMut<TileEvents>>,
    mut owed: Local<Vec<(Entity, Attribute)>>,
) {
    owed.clear();
    for (player, mut attributes, ..) in &mut players {
        for attribute in attributes.bypass_change_detection().take_dirty().iter() {
            if attribute.definition().syncable {
                owed.push((player, attribute));
            }
        }
    }
    owed.sort_unstable();
    if let Some(events) = events.as_deref_mut() {
        publish_changes(&owed, &players, events);
    }
    for announcement in announced.read() {
        let Ok((_, attributes, ..)) = players.get_mut(announcement.player) else {
            continue;
        };
        if !attributes.is_added() {
            continue;
        }
        for instance in attributes.set().instances() {
            if instance.attribute().definition().syncable {
                owed.push((announcement.player, instance.attribute()));
            }
        }
    }
    owed.sort_unstable();
    owed.dedup();
    for entries in owed.chunk_by(|left, right| left.0 == right.0) {
        let Some(&(player, _)) = entries.first() else {
            continue;
        };
        let Ok((_, attributes, &Owner(session), _)) = players.get(player) else {
            continue;
        };
        let snapshots: Vec<AttributeSnapshot<'_>> = entries
            .iter()
            .filter_map(|&(_, attribute)| attributes.snapshot(attribute))
            .collect();
        if snapshots.is_empty() {
            continue;
        }
        let packet = ClientboundUpdateAttributes {
            entity_id: VarInt(player_network_id(session)),
            attributes: snapshots,
        };
        let mut bytes = BytesMut::new().writer();
        match packet.encode_with_id(&mut bytes) {
            Ok(()) => {
                let packet = bytes.into_inner().freeze();
                writer.write(to(session, PacketPayload::OwnEntityState(packet)));
            }
            Err(error) => {
                tracing::warn!(
                    ?session,
                    "the attributes of a player did not encode: {error}"
                );
            }
        }
    }
}

/// Records the state of each instance of `owed` of a published player in its tile. A player that
/// is not published yet carries its instances in its spawn.
fn publish_changes(
    owed: &[(Entity, Attribute)],
    players: &Query<
        (Entity, &mut AttributeInstances, &Owner, Option<&Replicated>),
        Changed<AttributeInstances>,
    >,
    events: &mut TileEvents,
) {
    for &(player, attribute) in owed {
        let Ok((_, attributes, _, Some(replicated))) = players.get(player) else {
            continue;
        };
        let instance = attributes.set().instance(attribute).cloned();
        events.push(
            replicated.tile,
            replicated.id,
            EntityEvent::Data(
                PlayerData::Attribute(AttributeState {
                    attribute,
                    instance,
                })
                .into(),
            ),
        );
    }
}
