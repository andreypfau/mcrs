//! Per-dim block-update wire emit. Replaces the old host-side
//! `update_client_blocks` body (which queried `&mut ServerSideConnection`
//! directly) with a per-dim system that resolves recipients through
//! `Column.PlayerObservers` and emits one `OutboundPlayerPacket` per block
//! change. Single message hop — no two-frame buffer rotation across the
//! `World` boundary that caused the TNT silent-drop regression.
//!
//! Lives in the minecraft crate (not in `mcrs_minecraft_block`) to avoid
//! `mcrs_minecraft_block -> mcrs_minecraft_server` cycle. The block crate keeps
//! `BlockUpdatePlugin` (message types, set-block reader, SystemSet
//! definitions); this module supplies the additional
//! `BlockUpdateWirePlugin` that registers the new per-dim wire-emit
//! system in `FixedPostUpdate`.

use bevy_app::{App, FixedPostUpdate, Plugin};
use bevy_ecs::message::{MessageReader, MessageWriter};
use bevy_ecs::prelude::{Entity, IntoScheduleConfigs, Local, Query, With};
use mcrs_minecraft_core::BlockPos;
use mcrs_minecraft_core::ColumnPos;
use mcrs_minecraft_core::LocalPos;
use mcrs_minecraft_core::SectionPos;
use mcrs_minecraft_level::aoi::PlayerObservers;
use mcrs_minecraft_level::block::BlockUpdateFlags;
use mcrs_minecraft_level::block_update::BlockPlaced;
use mcrs_minecraft_level::entity::player::Player;
use mcrs_minecraft_level::palette::ChunkBlocks;
use mcrs_minecraft_level::session::PlayerSession;
use mcrs_minecraft_level::voxel_update::VoxelUpdateSet;
use mcrs_minecraft_level::world::dimension::InDimension;
use mcrs_minecraft_level::world::storage::column::ColumnIndex;
use rustc_hash::{FxHashMap, FxHashSet};
use smallvec::SmallVec;

pub use mcrs_minecraft_level::block_update::BlockUpdatePlugin;

use crate::world::bus::{OutboundPlayerPacket, PacketPayload, PacketPriority, PacketTarget};
use crate::world::entity::player::HostAnchor;

/// Per-dim wire emitter. Reads the blocks placed this tick that notify
/// clients, resolves the observer set through the section's column
/// (`ColumnPos::from(section_pos)` -> `ColumnIndex.0.get` -> column entity ->
/// `PlayerObservers`), and emits one `OutboundPlayerPacket { target: PlayerSet,
/// priority: Normal, data: PacketPayload::BlockUpdate { position, new_state } }`
/// per changed block.
///
/// Recipients are resolved at emit time by reading `PlayerObservers` on
/// the section's column entity rather than at consume time on the host.
/// A block placed more than once in a tick goes out once, with the state it
/// holds when the packet is built.
#[cfg_attr(
    feature = "telemetry-tracy",
    tracing::instrument(name = "block_update::update_client_blocks_per_dim", skip_all)
)]
pub fn update_client_blocks_per_dim(
    mut placed: MessageReader<BlockPlaced>,
    sections: Query<(&InDimension, &ChunkBlocks)>,
    column_indices: Query<&ColumnIndex>,
    observers: Query<&PlayerObservers>,
    live_players: Query<Entity, With<Player>>,
    anchors: Query<&HostAnchor>,
    mut packet_writer: MessageWriter<OutboundPlayerPacket>,
    mut changed: Local<FxHashMap<Entity, (SectionPos, FxHashSet<BlockPos>)>>,
) {
    for placed in placed.read() {
        if placed.flags.contains(BlockUpdateFlags::CLIENTS) {
            changed
                .entry(placed.chunk)
                .or_insert_with(|| (placed.chunk_pos, FxHashSet::default()))
                .1
                .insert(placed.block_pos);
        }
    }

    for (section, (section_pos, positions)) in changed.drain() {
        let Ok((in_dim, palette)) = sections.get(section) else {
            continue;
        };

        let column_pos = ColumnPos::from(section_pos);
        let mut observer_entities: SmallVec<[Entity; 8]> = column_indices
            .get(in_dim.0)
            .ok()
            .and_then(|idx| idx.0.get(&column_pos).map(|slot| slot.entity))
            .and_then(|column_entity| observers.get(column_entity).ok())
            .map(|obs| obs.0.iter().copied().collect())
            .unwrap_or_default();

        crate::world::aoi::retain_live_observers(&mut observer_entities, &live_players);

        // The anchor, not the dimension world's player entity: the session
        // registry is keyed by anchor, and a target it cannot resolve is
        // dropped without a trace.
        let targets: SmallVec<[Entity; 8]> = observer_entities
            .iter()
            .filter_map(|observer| anchors.get(*observer).ok().map(|anchor| anchor.0))
            .collect();

        if targets.is_empty() {
            continue;
        }

        for position in positions {
            let new_state = palette.get(LocalPos::from(position)).into();
            packet_writer.write(OutboundPlayerPacket {
                target: PacketTarget::PlayerSet(targets.clone()),
                priority: PacketPriority::Normal,
                data: PacketPayload::BlockUpdate {
                    position,
                    new_state,
                },
                session: PlayerSession(0),
                epoch: 0,
            });
        }
    }
}

/// Per-dim wire-emit plugin. Pairs with `BlockUpdatePlugin` (which
/// remains in the block crate and supplies the message types and the
/// set-block reader); both are registered into each `DimSubApp`.
pub struct BlockUpdateWirePlugin;

impl Plugin for BlockUpdateWirePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            FixedPostUpdate,
            update_client_blocks_per_dim.in_set(VoxelUpdateSet::NetworkSync),
        );
    }
}
