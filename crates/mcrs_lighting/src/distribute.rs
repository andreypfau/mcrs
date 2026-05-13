//! Cross-section distribute pass: drains `*Egress` wavefronts from source
//! sections and pre-attenuates them onto the destination section's
//! `*Incoming` (or `*PendingEgress` on overflow).
//!
//! Face-direction contract:
//!
//! * BFS emits to `*Egress` with `face = direction-from-source-cell OUT of
//!   the source section` (source frame).
//! * Distribute pre-attenuates and writes to the destination's `*Incoming`
//!   with `face = direction-from-destination-cell IN from the source
//!   section` (destination frame — the opposite of the source's face).
//! * The neighbor-edge pull system reuses the destination-frame convention.
//!
//! Two-pass shape (mandated by Bevy's borrow checker — a single query
//! cannot hold `&mut *Egress` on a source and `&mut *Incoming` on a
//! destination simultaneously when both may resolve to the same entity at
//! adjacent ticks): Pass A drains `*Egress` into a `Local` staging buffer,
//! Pass B applies staged wavefronts to `*Incoming`, Pass C inserts the
//! `LightDirty` marker on each unique destination via a `Local` dedup set.
//!
//! Bodies are placeholders at this point; this module currently ships
//! only the symbol surface and the pre-attenuation helper.
use bevy_ecs::entity::EntityHashSet;
use bevy_ecs::prelude::*;
use std::sync::atomic::Ordering;
use std::time::Instant;

use crate::bfs::project_face_cell;
use crate::components::{
    BlockEgress, BlockIncoming, BlockPendingEgress, LightDirty, NeedsFullReseed, SkyEgress,
    SkyIncoming, SkyPendingEgress, Wavefront,
};
use crate::converge::PENDING_EGRESS_CAP;
use crate::telemetry::{
    LIGHT_CROSS_DIM_VIOLATIONS_TOTAL, LIGHT_PENDING_EGRESS_OVERFLOW_TOTAL,
};
use mcrs_core::voxel_shape::Direction;
use mcrs_engine::world::chunk::ChunkPos;
use mcrs_engine::world::column::{ColumnIndex, InChunkColumn, SectionIndex, SectionLookup};
use mcrs_engine::world::dimension::InDimension;
use mcrs_engine::world::lighting::LightTicket;

#[inline]
pub(crate) fn manhattan_preattenuate(level: u8, adjacency: u8) -> u8 {
    level.saturating_sub(adjacency.max(1))
}

pub(crate) fn resolve_neighbor_section(
    _src_section: Entity,
    _direction: Direction,
    _column_index: &ColumnIndex,
    _section_indices: &Query<&SectionIndex>,
    _section_pos: &Query<(&ChunkPos, &InDimension, &InChunkColumn)>,
) -> Option<(Entity, ChunkPos)> {
    None
}

pub fn distribute_decrease(
    _block_egress: Query<&mut BlockEgress>,
    _sky_egress: Query<&mut SkyEgress>,
    _block_incoming: Query<&mut BlockIncoming>,
    _sky_incoming: Query<&mut SkyIncoming>,
    _block_pending: Query<&mut BlockPendingEgress>,
    _sky_pending: Query<&mut SkyPendingEgress>,
    _section_pos: Query<(&ChunkPos, &InDimension, &InChunkColumn)>,
    _section_indices: Query<&SectionIndex>,
    _column_indices: Query<&ColumnIndex>,
    _light_tickets: Query<(), With<LightTicket>>,
    _block_stage: Local<Vec<(Entity, Wavefront)>>,
    _sky_stage: Local<Vec<(Entity, Wavefront)>>,
    _dirty_dedup: Local<EntityHashSet>,
    _last_xdim_log: Local<Option<Instant>>,
    _commands: Commands,
) {
}

pub fn distribute_increase(
    _block_egress: Query<&mut BlockEgress>,
    _sky_egress: Query<&mut SkyEgress>,
    _block_incoming: Query<&mut BlockIncoming>,
    _sky_incoming: Query<&mut SkyIncoming>,
    _block_pending: Query<&mut BlockPendingEgress>,
    _sky_pending: Query<&mut SkyPendingEgress>,
    _section_pos: Query<(&ChunkPos, &InDimension, &InChunkColumn)>,
    _section_indices: Query<&SectionIndex>,
    _column_indices: Query<&ColumnIndex>,
    _light_tickets: Query<(), With<LightTicket>>,
    _block_stage: Local<Vec<(Entity, Wavefront)>>,
    _sky_stage: Local<Vec<(Entity, Wavefront)>>,
    _dirty_dedup: Local<EntityHashSet>,
    _last_xdim_log: Local<Option<Instant>>,
    _commands: Commands,
) {
}

#[allow(dead_code)]
fn _unused_imports_anchor() {
    // Anchor symbols that the real distribute bodies will consume so the
    // imports do not get pruned while the bodies are still placeholders.
    let _ = project_face_cell;
    let _ = SectionLookup::Loaded;
    let _ = NeedsFullReseed;
    let _ = LightDirty;
    let _ = LIGHT_PENDING_EGRESS_OVERFLOW_TOTAL.load(Ordering::Relaxed);
    let _ = LIGHT_CROSS_DIM_VIOLATIONS_TOTAL.load(Ordering::Relaxed);
    let _ = PENDING_EGRESS_CAP;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manhattan_preattenuate_face_edge_corner() {
        assert_eq!(manhattan_preattenuate(15, 1), 14);
        assert_eq!(manhattan_preattenuate(15, 2), 13);
        assert_eq!(manhattan_preattenuate(15, 3), 12);
        assert_eq!(manhattan_preattenuate(3, 5), 0);
    }
}
