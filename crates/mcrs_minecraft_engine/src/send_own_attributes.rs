use bevy_ecs::change_detection::{DetectChanges, Ref};
use bevy_ecs::entity::Entity;
use bevy_ecs::message::{MessageReader, MessageWriter};
use bevy_ecs::schedule::SystemSet;
use bevy_ecs::system::{Local, Query, ResMut};
use mcrs_engine_muxer::EntityEvent;
use mcrs_minecraft_engine_attributes::{AttributeChanged, AttributeTarget, Attributes};
use mcrs_minecraft_engine_world::session::Owner;
use mcrs_minecraft_entity::attribute_keys::Attribute;
use mcrs_minecraft_protocol::VarInt;
use mcrs_minecraft_replication::{AttributeState, PlayerData, own_attributes};

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
/// probes of its prediction (MVC-PKT-031). It holds the state of each syncable instance that an
/// edit changed in the tick (ATR-NET-003), and, for a player that a `login` or a `respawn` just
/// placed, each stored syncable instance (ATR-NET-005). It also records the state of each changed
/// instance of a published player for the players that track it (ATR-NET-003).
// A placed player is a new entity whose operator level is announced with the `login` or the
// `respawn`, which went out before the tick that runs this system.
pub(crate) fn send_own_attributes(
    mut changed: MessageReader<AttributeChanged>,
    mut announced: MessageReader<OpLevelAnnounced>,
    players: Query<(Ref<Attributes>, &Owner, Option<&Replicated>)>,
    mut writer: MessageWriter<OutboundPlayerPacket>,
    mut events: Option<ResMut<TileEvents>>,
    mut owed: Local<Vec<(Entity, Attribute)>>,
) {
    owed.clear();
    for change in changed.read() {
        if let AttributeTarget::Player(player) = change.target
            && change.attribute.definition().syncable
        {
            owed.push((player, change.attribute));
        }
    }
    owed.sort_unstable();
    owed.dedup();
    if let Some(events) = events.as_deref_mut() {
        publish_changes(&owed, &players, events);
    }
    for announcement in announced.read() {
        let Ok((attributes, ..)) = players.get(announcement.player) else {
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
        let Ok((attributes, &Owner(session), _)) = players.get(player) else {
            continue;
        };
        let id = VarInt(player_network_id(session));
        let owed_attributes = entries.iter().map(|&(_, attribute)| attribute);
        match own_attributes(id, attributes.set(), owed_attributes) {
            Ok(Some(packet)) => {
                writer.write(to(session, PacketPayload::OwnEntityState(packet)));
            }
            Ok(None) => {}
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
    players: &Query<(Ref<Attributes>, &Owner, Option<&Replicated>)>,
    events: &mut TileEvents,
) {
    for &(player, attribute) in owed {
        let Ok((attributes, _, Some(replicated))) = players.get(player) else {
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
