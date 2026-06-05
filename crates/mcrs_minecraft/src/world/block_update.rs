//! Per-dim block-update wire emit. Replaces the old host-side
//! `update_client_blocks` body (which queried `&mut ServerSideConnection`
//! directly) with a per-dim system that resolves recipients through
//! `Column.PlayerObservers` and emits one `OutboundPlayerPacket` per block
//! change. Single message hop — no two-frame buffer rotation across the
//! `World` boundary that caused the TNT silent-drop regression.
//!
//! Lives in the minecraft crate (not in `mcrs_minecraft_block`) to avoid
//! `mcrs_minecraft_block -> mcrs_minecraft` cycle. The block crate keeps
//! `BlockUpdatePlugin` (message types, set-block reader, change-tracker
//! seeder, SystemSet definitions); this module supplies the additional
//! `BlockUpdateWirePlugin` that registers the new per-dim wire-emit
//! system in `FixedPostUpdate`.

use bevy_app::{App, FixedPostUpdate, Plugin};
use bevy_ecs::message::MessageWriter;
use bevy_ecs::prelude::{Changed, Entity, IntoScheduleConfigs, Query, With};
use mcrs_engine::aoi::PlayerObservers;
use mcrs_engine::entity::player::Player;
use mcrs_engine::geometry::ColumnPos;
use mcrs_engine::world::chunk::ChunkPos;
use mcrs_engine::world::dimension::InDimension;
use mcrs_engine::world::storage::column::ColumnIndex;
use mcrs_minecraft_block::block_update::{BlockUpdateSet, ChunkNetworkSyncBlockChangesSet};
use mcrs_minecraft_block::palette::BlockPalette;
use smallvec::SmallVec;

pub use mcrs_minecraft_block::block_update::BlockUpdatePlugin;

use std::sync::atomic::Ordering;

use crate::world::bus::{OutboundPlayerPacket, PacketPayload, PacketPriority, PacketTarget};

/// Per-dim wire emitter. Iterates chunks whose
/// `ChunkNetworkSyncBlockChangesSet` changed this tick, resolves the
/// observer set through the chunk's column (`ColumnPos::from(chunk_pos)`
/// -> `ColumnIndex.0.get` -> column entity -> `PlayerObservers`), and
/// emits one `OutboundPlayerPacket { target: PlayerSet, priority: Normal,
/// data: PacketPayload::BlockUpdate { position, new_state } }` per
/// changed block, then clears the changes set.
///
/// Recipients are resolved at emit time by reading `PlayerObservers` on
/// the chunk's column entity rather than at consume time on the host.
/// The change set is drained every tick — same lifecycle as the previous
/// host-side body, so repeated changes on the same block within a tick
/// coalesce into a single packet emit.
#[cfg_attr(
    feature = "telemetry-tracy",
    tracing::instrument(name = "block_update::update_client_blocks_per_dim", skip_all)
)]
pub fn update_client_blocks_per_dim(
    mut chunks: Query<
        (
            &ChunkPos,
            &InDimension,
            &BlockPalette,
            &mut ChunkNetworkSyncBlockChangesSet,
        ),
        Changed<ChunkNetworkSyncBlockChangesSet>,
    >,
    column_indices: Query<&ColumnIndex>,
    observers: Query<&PlayerObservers>,
    live_players: Query<Entity, With<Player>>,
    mut packet_writer: MessageWriter<OutboundPlayerPacket>,
) {
    for (chunk_pos, in_dim, palette, mut changes) in chunks.iter_mut() {
        if changes.changes.is_empty() {
            continue;
        }

        let column_pos = ColumnPos::from(*chunk_pos);
        let mut observer_entities: SmallVec<[Entity; 8]> = column_indices
            .get(in_dim.0)
            .ok()
            .and_then(|idx| idx.0.get(&column_pos).map(|slot| slot.entity))
            .and_then(|column_entity| observers.get(column_entity).ok())
            .map(|obs| obs.0.iter().copied().collect())
            .unwrap_or_default();

        crate::world::aoi::retain_live_observers(&mut observer_entities, &live_players);

        // Drain regardless of whether there are recipients — leaving stale
        // entries in the change set would re-fire `Changed<...>` next tick
        // and keep emitting empty packets, or accumulate unbounded.
        let positions: Vec<_> = changes.changes.drain().collect();

        if observer_entities.is_empty() {
            continue;
        }

        for position in positions {
            let new_state = palette.get(position);
            packet_writer.write(OutboundPlayerPacket {
                target: PacketTarget::PlayerSet(observer_entities.clone()),
                priority: PacketPriority::Normal,
                data: PacketPayload::BlockUpdate {
                    position,
                    new_state,
                },
            });
            mcrs_network::metrics::BRIDGE_OUTBOUND_MESSAGES_EMITTED_TOTAL
                .fetch_add(1, Ordering::Relaxed);
        }
    }
}

/// Per-dim wire-emit plugin. Pairs with `BlockUpdatePlugin` (which
/// remains in the block crate and supplies the message types + reader +
/// change-tracker seeder); both are registered into each `DimSubApp`.
pub struct BlockUpdateWirePlugin;

impl Plugin for BlockUpdateWirePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            FixedPostUpdate,
            update_client_blocks_per_dim.in_set(BlockUpdateSet::NetworkSync),
        );
    }
}
