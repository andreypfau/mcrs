use bevy_ecs::change_detection::{DetectChanges, DetectChangesMut};
use bevy_ecs::entity::Entity;
use bevy_ecs::message::{MessageReader, MessageWriter};
use bevy_ecs::query::Changed;
use bevy_ecs::schedule::SystemSet;
use bevy_ecs::system::{Local, Populated, Query, ResMut};
use mcrs_engine_muxer::EntityEvent;
use mcrs_minecraft_engine_world::session::Owner;
use mcrs_minecraft_entity_attribute::attribute_keys::Attribute;
use mcrs_minecraft_entity_attribute::{AttributeInstances, AttributeSnapshot};
use mcrs_minecraft_protocol::VarInt;
use mcrs_minecraft_protocol_game::clientbound::ClientboundUpdateAttributes;
use mcrs_minecraft_replication::{AttributeState, PlayerData};

use crate::ability::OpLevelAnnounced;
use crate::bus::{OutboundPlayerPacket, send_own_entity_state};
use crate::network_id::player_network_id;
use crate::replicated::Replicated;
use crate::tile_events::TileEvents;

#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct OwnAttributesSet;

pub(crate) fn send_own_attributes(
    mut announced: MessageReader<OpLevelAnnounced>,
    mut players: Populated<
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
        send_own_entity_state(&mut writer, session, &packet, "attributes");
    }
}

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
