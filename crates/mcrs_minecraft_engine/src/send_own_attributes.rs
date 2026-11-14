use bevy_ecs::change_detection::DetectChanges;
use bevy_ecs::change_detection::Ref;
use bevy_ecs::entity::Entity;
use bevy_ecs::message::MessageReader;
use bevy_ecs::message::MessageWriter;
use bevy_ecs::system::Local;
use bevy_ecs::system::Query;
use mcrs_minecraft_engine_attributes::AttributeChanged;
use mcrs_minecraft_engine_attributes::AttributeTarget;
use mcrs_minecraft_engine_attributes::Attributes;
use mcrs_minecraft_engine_world::session::Owner;
use mcrs_minecraft_entity::attribute_keys::Attribute;
use mcrs_minecraft_protocol::VarInt;
use mcrs_minecraft_replication::own_attributes;

use crate::ability::OpLevelAnnounced;
use crate::bus::{OutboundPlayerPacket, PacketPayload, to};
use crate::network_id::player_network_id;

/// Gives the client of each player one `update_attributes` about its own entity, inside the
/// probes of its prediction (MVC-PKT-031). It holds the state of each syncable instance that an
/// edit changed in the tick (ATR-NET-003), and, for a player that a `login` or a `respawn` just
/// placed, each stored syncable instance (ATR-NET-005).
// A placed player is a new entity whose operator level is announced with the `login` or the
// `respawn`, which went out before the tick that runs this system.
pub(crate) fn send_own_attributes(
    mut changed: MessageReader<AttributeChanged>,
    mut announced: MessageReader<OpLevelAnnounced>,
    players: Query<(Ref<Attributes>, &Owner)>,
    mut writer: MessageWriter<OutboundPlayerPacket>,
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
    for announcement in announced.read() {
        let Ok((attributes, _)) = players.get(announcement.player) else {
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
        let Ok((attributes, &Owner(session))) = players.get(player) else {
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
