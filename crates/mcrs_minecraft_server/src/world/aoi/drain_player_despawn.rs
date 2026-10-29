//! Per-dim drain system for player removals arriving via the inbound
//! lifecycle bus. When a player disconnects or changes dimension, the
//! main-world disconnect path pushes an `InboundPlayerDespawn` message
//! into `PendingInboundLifecycle`; the extract closure shuttles it into
//! the per-dim sub-app's `Messages<InboundPlayerDespawn>` buffer; and
//! this system, running in `FixedPreUpdate`, consumes those messages and
//! runs the full eviction sequence for the corresponding in-dim `Player`
//! entity.
//!
//! A defensive helper `retain_live_observers` is exposed for use by
//! per-dim wire emitters that want a belt-and-braces liveness filter
//! before fan-out.

use bevy_ecs::message::{MessageReader, MessageWriter};
use bevy_ecs::prelude::{Commands, Entity, In, Query, With};
use mcrs_minecraft_engine::bus::to;
use mcrs_minecraft_level::entity::player::Player;
use mcrs_minecraft_protocol::VarInt;
use mcrs_minecraft_protocol::packets::game::clientbound::ClientboundRemoveEntities;
use smallvec::SmallVec;

use crate::world::aoi::components::TrackedBy;
use crate::world::entity::player::column_view::ColumnView;
use mcrs_minecraft_engine::bus::{InboundPlayerDespawn, OutboundPlayerPacket, PacketPayload};
use mcrs_minecraft_engine::session::HostAnchor;

/// Per-dim drain: reads `InboundPlayerDespawn` messages and runs the full
/// eviction sequence for the resolved in-dim `Player`:
///
/// 1. Emit `PlayerLeftView` to every in-dim player whose `TrackedBy`
///    currently contains the removed entity (former observers). Captured
///    before any cache is wiped so the recipient list is complete.
/// 2. Single pass over every in-dim player's `TrackedBy`: non-target rows
///    get `retain(|e| *e != target)` (proactive cache eviction); the target
///    row is cleared (self-teardown).
/// 3. Take the removed player's `ColumnView`, which hands back its tickets
///    and withdraws it from every column it held.
#[cfg_attr(
    feature = "telemetry-tracy",
    tracing::instrument(name = "aoi::drain_inbound_player_despawn", skip_all)
)]
pub fn drain_inbound_player_despawn(
    mut despawn_msgs: MessageReader<InboundPlayerDespawn>,
    player_lookup: Query<(Entity, &HostAnchor), With<Player>>,
    mut player_caches: Query<&mut TrackedBy, With<Player>>,
    mut packet_writer: MessageWriter<OutboundPlayerPacket>,
    mut commands: Commands,
) {
    for msg in despawn_msgs.read() {
        let host_anchor = msg.host_anchor;
        let in_dim_entity = player_lookup
            .iter()
            .find_map(|(e, anchor)| (anchor.0 == host_anchor).then_some(e));
        let Some(target) = in_dim_entity else {
            // No player of this dimension carries the anchor yet: the disconnect came before
            // the spawn of the player here. There is nothing to evict.
            continue;
        };

        evict(
            target,
            host_anchor,
            &player_lookup,
            &mut player_caches,
            &mut packet_writer,
        );
        commands.entity(target).try_remove::<ColumnView>();
    }
}

/// A player that leaves this dimension for another one, through the same eviction as a
/// player that disconnects; the caller despawns it.
pub(crate) fn leave_dimension(
    In(target): In<Entity>,
    player_lookup: Query<(Entity, &HostAnchor), With<Player>>,
    mut player_caches: Query<&mut TrackedBy, With<Player>>,
    mut packet_writer: MessageWriter<OutboundPlayerPacket>,
) {
    let Ok((_, &HostAnchor(host_anchor))) = player_lookup.get(target) else {
        return;
    };
    evict(
        target,
        host_anchor,
        &player_lookup,
        &mut player_caches,
        &mut packet_writer,
    );
}

fn evict(
    target: Entity,
    host_anchor: Entity,
    player_lookup: &Query<(Entity, &HostAnchor), With<Player>>,
    player_caches: &mut Query<&mut TrackedBy, With<Player>>,
    packet_writer: &mut MessageWriter<OutboundPlayerPacket>,
) {
    let left_view = ClientboundRemoveEntities {
        entity_ids: vec![VarInt(
            mcrs_minecraft_engine::network_id::player_network_id(host_anchor),
        )],
    };

    // The former observers are found through their TrackedBy, so they hear of the leave before
    // the pass below removes `target` from it; update_tracked_by runs later and never sees it.
    for (observer_entity, &HostAnchor(observer)) in player_lookup.iter() {
        if observer_entity == target {
            continue;
        }
        if let Ok(tracked_by) = player_caches.get(observer_entity)
            && tracked_by.0.contains(&target)
        {
            packet_writer.write(to(
                observer,
                PacketPayload::PlayerLeftView(left_view.clone()),
            ));
        }
    }

    for (entity, _) in player_lookup.iter() {
        if let Ok(mut tracked_by) = player_caches.get_mut(entity) {
            if entity == target {
                tracked_by.0.clear();
            } else {
                tracked_by.0.retain(|e| *e != target);
            }
        }
    }
}

/// Defensive helper: filter an `observer_entities` snapshot against a
/// live-player query before fan-out. Used by per-dim wire emitters
/// (`update_client_blocks_per_dim`) as belt-and-braces protection
/// against any future code path that mutates `PlayerObservers` without
/// going through the drain above. Cost: one Query iteration per
/// emit-site invocation; acceptable because the live-player set is small
/// and the emit site already pays a Query cost for `&PlayerObservers`.
pub fn retain_live_observers(
    observer_entities: &mut SmallVec<[Entity; 8]>,
    live_players: &Query<Entity, With<Player>>,
) {
    observer_entities.retain(|entity| live_players.get(*entity).is_ok());
}
