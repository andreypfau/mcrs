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

use std::sync::atomic::Ordering;

use bevy_ecs::message::{MessageReader, MessageWriter};
use bevy_ecs::prelude::{Entity, Query, With, Without};
use mcrs_engine::aoi::PlayerObservers;
use mcrs_engine::entity::player::Player;
use mcrs_engine::world::storage::column::Column;
use smallvec::SmallVec;

use crate::world::aoi::components::{ChunkSubscriptionSet, TrackedBy};
use crate::world::bus::{
    InboundPlayerDespawn, OutboundPlayerPacket, PacketPayload, PacketPriority, PacketTarget,
};
use crate::world::entity::player::HostAnchor;

/// Per-dim drain: reads `InboundPlayerDespawn` messages and runs the full
/// eviction sequence for the resolved in-dim `Player`:
///
/// 1. Emit `PlayerLeftView` to every in-dim player whose `TrackedBy`
///    currently contains the removed entity (former observers). Captured
///    before any cache is wiped so the recipient list is complete.
/// 2. Single pass over every in-dim player's `TrackedBy`: non-target rows
///    get `retain(|e| *e != target)` (proactive cache eviction); the target
///    row gets both caches cleared (self-teardown).
/// 3. Retain the removed entity out of every column's `PlayerObservers`
///    (existing behaviour, preserved).
///
/// The query filter `(With<Column>, Without<Player>)` mirrors
/// `update_own_pov` and `update_tracked_by` — it makes the disjoint-borrow
/// claim explicit and prevents accidental matches on any future entity that
/// carries both `Column` and `PlayerObservers`.
#[cfg_attr(
    feature = "telemetry-tracy",
    tracing::instrument(name = "aoi::drain_inbound_player_despawn", skip_all)
)]
pub fn drain_inbound_player_despawn(
    mut despawn_msgs: MessageReader<InboundPlayerDespawn>,
    player_lookup: Query<(Entity, &HostAnchor), With<Player>>,
    mut columns: Query<&mut PlayerObservers, (With<Column>, Without<Player>)>,
    mut player_caches: Query<(&mut TrackedBy, &mut ChunkSubscriptionSet), With<Player>>,
    mut packet_writer: MessageWriter<OutboundPlayerPacket>,
) {
    for msg in despawn_msgs.read() {
        let host_anchor = msg.host_anchor;
        let in_dim_entity = player_lookup
            .iter()
            .find_map(|(e, anchor)| (anchor.0 == host_anchor).then_some(e));
        let Some(target) = in_dim_entity else {
            // No per-dim Player carries this host_anchor — happens when the
            // disconnect message arrives for a dim the player never reached
            // (mid-transit disconnect into the previous_dim bundle), or
            // before the per-dim spawn consumer has wired HostAnchor onto
            // the in-dim Player. Harmless: there is nothing to evict here.
            continue;
        };

        let entity_ids: SmallVec<[i32; 4]> = SmallVec::from_slice(&[target.index_u32() as i32]);

        // Emit PlayerLeftView to former observers before any cache is mutated.
        // Former observers are all in-dim players whose TrackedBy currently
        // contains `target`. Shared borrow of player_caches here; the loop
        // completes before the mutable pass below.
        //
        // This must run inside the drain rather than update_tracked_by because
        // the drain wipes PlayerObservers and TrackedBy before update_tracked_by
        // ever runs (FixedPreUpdate vs FixedPostUpdate), so update_tracked_by
        // will never see the left-view transition for a removed player.
        for (observer_entity, _) in player_lookup.iter() {
            if observer_entity == target {
                continue;
            }
            if let Ok((tracked_by, _)) = player_caches.get(observer_entity) {
                if tracked_by.0.contains(&target) {
                    packet_writer.write(OutboundPlayerPacket {
                        target: PacketTarget::SinglePlayer(observer_entity),
                        priority: PacketPriority::Normal,
                        data: PacketPayload::PlayerLeftView {
                            entity_ids: entity_ids.clone(),
                        },
                    });
                    mcrs_network::metrics::BRIDGE_OUTBOUND_MESSAGES_EMITTED_TOTAL
                        .fetch_add(1, Ordering::Relaxed);
                }
            }
        }

        // Single mutable pass over all in-dim player caches.
        // Target's own row: clear both caches (self-teardown).
        // Every other row: retain-remove target from TrackedBy (proactive eviction).
        for (entity, _) in player_lookup.iter() {
            if let Ok((mut tracked_by, mut subs)) = player_caches.get_mut(entity) {
                if entity == target {
                    tracked_by.0.clear();
                    subs.0.clear();
                } else {
                    tracked_by.0.retain(|e| *e != target);
                }
            }
        }

        // Retain target out of every column's PlayerObservers.
        for mut obs in columns.iter_mut() {
            obs.0.retain(|e| *e != target);
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
