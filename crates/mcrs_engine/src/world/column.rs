// IMPORTANT: this module MUST NOT depend on mcrs_lighting. The four-stage
// ChunkColumnLifecycleSet splits into engine-side (Reconcile, ReconcileIndex) and
// lighting-side (PrimeHeightmaps, AttachState) stages precisely because mcrs_engine
// sits upstream of mcrs_lighting in the workspace graph.
//
// Heightmaps zero-init convention: `Heightmaps::new(height)` zero-initializes the
// backing PackedBitStorage long arrays. `surface_get(x, z) = min_y` for unprimed
// columns; downstream lighting code overwrites with real values before any consumer
// reads, and uses `min_y` as the "no surface found" sentinel.

use crate::world::block::BlockPos;
use crate::world::chunk::{ChunkLoaded, ChunkPos, ChunkUnloading};
use crate::world::dimension::{DimensionTypeConfig, InDimension};
use bevy_app::{App, FixedUpdate, Plugin};
use bevy_derive::{Deref, DerefMut};
use bevy_ecs::prelude::{
    Added, ApplyDeferred, Bundle, Commands, Component, Entity, IntoScheduleConfigs, Query,
    SystemSet,
};
use bevy_math::IVec2;
use rustc_hash::FxHashMap;
use std::fmt::Debug;

/// The XZ position of a chunk column. Engine-local twin of the wire-side
/// `mcrs_protocol::ChunkColumnPos`; defined here so the engine has a column-key
/// type without depending on the protocol crate.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Hash)]
pub struct ChunkColumnPos {
    pub x: i32,
    pub z: i32,
}

impl Debug for ChunkColumnPos {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        (self.x, self.z).fmt(f)
    }
}

impl ChunkColumnPos {
    pub const fn new(x: i32, z: i32) -> Self {
        Self { x, z }
    }
}

impl From<ChunkPos> for ChunkColumnPos {
    fn from(pos: ChunkPos) -> Self {
        Self { x: pos.x, z: pos.z }
    }
}

impl From<BlockPos> for ChunkColumnPos {
    fn from(pos: BlockPos) -> Self {
        Self {
            x: pos.x.div_euclid(16),
            z: pos.z.div_euclid(16),
        }
    }
}

impl From<IVec2> for ChunkColumnPos {
    fn from(v: IVec2) -> Self {
        Self { x: v.x, z: v.y }
    }
}

impl From<(i32, i32)> for ChunkColumnPos {
    fn from((x, z): (i32, i32)) -> Self {
        Self { x, z }
    }
}

/// Sparse marker component placed on chunk-column entities.
#[derive(Component, Debug, Default)]
#[component(storage = "SparseSet")]
pub struct ChunkColumn;

/// Back-link from a ChunkSection entity to its owning ChunkColumn entity.
/// Inserted by `reconcile_section_index` (Stage 2).
#[derive(Component, Clone, Copy, Debug)]
pub struct InChunkColumn(pub Entity);

/// Per-column entry in `ColumnIndex`.
#[derive(Debug, Clone, Copy)]
pub struct ColumnSlot {
    pub entity: Entity,
    pub section_count: u32,
}

/// Per-dimension lookup from `ChunkColumnPos` to the column entity + refcount.
/// Lives on the Dimension entity (added as a `DimensionBundle` field).
#[derive(Component, Debug, Default, Deref, DerefMut)]
pub struct ColumnIndex(pub FxHashMap<ChunkColumnPos, ColumnSlot>);

/// Packed-bit storage backing for heightmaps. Each `u64` long holds
/// `entries_per_long = 64 / bits_per_entry` entries; the lowest entry occupies
/// the lowest bits of each long (matches the Minecraft `SimpleBitStorage` wire
/// format).
#[derive(Debug, Clone)]
pub struct PackedBitStorage {
    longs: Vec<u64>,
    bits_per_entry: u8,
    entries_per_long: u8,
    entry_count: u32,
    max_value: u32,
}

impl PackedBitStorage {
    pub fn new(entry_count: usize, max_value: u32) -> Self {
        let bits_per_entry = bits_needed_for(max_value);
        Self::with_bits(entry_count, bits_per_entry, max_value)
    }

    fn with_bits(entry_count: usize, bits_per_entry: u8, max_value: u32) -> Self {
        debug_assert!(
            bits_per_entry > 0 && bits_per_entry <= 32,
            "bits_per_entry must be in 1..=32 (got {bits_per_entry})"
        );
        let entries_per_long = (64 / bits_per_entry as u32) as u8;
        let longs_needed = entry_count.div_ceil(entries_per_long as usize);
        Self {
            longs: vec![0u64; longs_needed],
            bits_per_entry,
            entries_per_long,
            entry_count: entry_count as u32,
            max_value,
        }
    }

    #[inline]
    pub fn get(&self, index: usize) -> u32 {
        debug_assert!(
            index < self.entry_count as usize,
            "PackedBitStorage::get index {index} out of range (entry_count={})",
            self.entry_count
        );
        let entries_per_long = self.entries_per_long as usize;
        let long_index = index / entries_per_long;
        let sub_index = index % entries_per_long;
        let shift = sub_index as u32 * self.bits_per_entry as u32;
        let mask: u64 = if self.bits_per_entry == 64 {
            u64::MAX
        } else {
            (1u64 << self.bits_per_entry) - 1
        };
        ((self.longs[long_index] >> shift) & mask) as u32
    }

    #[inline]
    pub fn set(&mut self, index: usize, value: u32) {
        debug_assert!(
            index < self.entry_count as usize,
            "PackedBitStorage::set index {index} out of range (entry_count={})",
            self.entry_count
        );
        debug_assert!(
            value <= self.max_value,
            "PackedBitStorage::set value {value} exceeds max_value {}",
            self.max_value
        );
        let entries_per_long = self.entries_per_long as usize;
        let long_index = index / entries_per_long;
        let sub_index = index % entries_per_long;
        let shift = sub_index as u32 * self.bits_per_entry as u32;
        let mask: u64 = if self.bits_per_entry == 64 {
            u64::MAX
        } else {
            (1u64 << self.bits_per_entry) - 1
        };
        let cleared = self.longs[long_index] & !(mask << shift);
        self.longs[long_index] = cleared | ((value as u64 & mask) << shift);
    }

    pub fn raw_longs(&self) -> &[u64] {
        &self.longs
    }

    pub fn bits_per_entry(&self) -> u8 {
        self.bits_per_entry
    }

    pub fn entry_count(&self) -> u32 {
        self.entry_count
    }
}

/// `ceil(log2(max_value + 1))` clamped to a minimum of 1.
fn bits_needed_for(max_value: u32) -> u8 {
    if max_value == 0 {
        return 1;
    }
    (32 - max_value.leading_zeros()) as u8
}

/// World-surface and motion-blocking heightmaps for a chunk column. Indexed
/// by `(x, z)` in `0..16` each (entry layout matches vanilla
/// `Heightmap.java`: `z * 16 + x`). Stored Y values are absolute world Y.
#[derive(Component, Debug, Clone)]
pub struct Heightmaps {
    pub world_surface: PackedBitStorage,
    pub motion_blocking: PackedBitStorage,
    height: u32,
    min_y: i32,
}

impl Heightmaps {
    /// Create heightmaps sized to the dimension height. `min_y` defaults to 0;
    /// use `with_min_y` for dimensions whose lowest section is negative.
    pub fn new(height: u32) -> Self {
        Self::with_min_y(height, 0)
    }

    pub fn with_min_y(height: u32, min_y: i32) -> Self {
        let max_value = height; // stored value range is [0, height]
        let bits = bits_needed_for(max_value);
        Self {
            world_surface: PackedBitStorage::with_bits(256, bits, max_value),
            motion_blocking: PackedBitStorage::with_bits(256, bits, max_value),
            height,
            min_y,
        }
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn min_y(&self) -> i32 {
        self.min_y
    }

    #[inline]
    fn index(x: usize, z: usize) -> usize {
        debug_assert!(x < 16 && z < 16, "Heightmaps index ({x}, {z}) out of 16x16");
        (z & 15) * 16 + (x & 15)
    }

    pub fn surface_get(&self, x: usize, z: usize) -> i32 {
        self.world_surface.get(Self::index(x, z)) as i32 + self.min_y
    }

    pub fn surface_set(&mut self, x: usize, z: usize, y: i32) {
        let max_stored = self.min_y + self.height as i32;
        debug_assert!(
            y >= self.min_y && y <= max_stored,
            "surface_set y={y} outside [{min}, {max}]",
            min = self.min_y,
            max = max_stored,
        );
        let rel = (y - self.min_y).clamp(0, self.height as i32);
        self.world_surface.set(Self::index(x, z), rel as u32);
    }

    pub fn motion_blocking_get(&self, x: usize, z: usize) -> i32 {
        self.motion_blocking.get(Self::index(x, z)) as i32 + self.min_y
    }

    pub fn motion_blocking_set(&mut self, x: usize, z: usize, y: i32) {
        let max_stored = self.min_y + self.height as i32;
        debug_assert!(
            y >= self.min_y && y <= max_stored,
            "motion_blocking_set y={y} outside [{min}, {max}]",
            min = self.min_y,
            max = max_stored,
        );
        let rel = (y - self.min_y).clamp(0, self.height as i32);
        self.motion_blocking.set(Self::index(x, z), rel as u32);
    }

    pub fn to_long_array_surface(&self) -> &[u64] {
        self.world_surface.raw_longs()
    }

    pub fn to_long_array_motion_blocking(&self) -> &[u64] {
        self.motion_blocking.raw_longs()
    }
}

// No `Default for Heightmaps`: every column ships with a dimension-shape
// derived size via `Heightmaps::with_min_y`, and a 384-tall hardcoded default
// would silently mis-size storage for the nether (height 256) or end (height
// 256). Callers that need a fresh heightmap must go through
// `DimensionTypeConfig` so the right shape is plumbed in.

/// Result of looking up a chunk-section by `chunk_y` inside a column.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SectionLookup {
    Loaded(Entity),
    Unloaded,
    BottomPadding,
    TopPadding,
    OutOfRange,
}

/// Per-column index of real chunk-section entities. The backing storage holds
/// `real_count` entries (no padding); `iter_wire()` adds the two padding rows
/// expected by the network wire format.
#[derive(Component, Debug, Clone)]
pub struct SectionIndex {
    pub min_section_y: i32,
    pub sections: Box<[Option<Entity>]>,
}

impl SectionIndex {
    pub fn new(min_section_y: i32, real_count: usize) -> Self {
        Self {
            min_section_y,
            sections: vec![None; real_count].into_boxed_slice(),
        }
    }

    pub fn lookup(&self, chunk_y: i32) -> SectionLookup {
        let rel = chunk_y - self.min_section_y;
        let len = self.sections.len() as i32;
        if rel == -1 {
            return SectionLookup::BottomPadding;
        }
        if rel == len {
            return SectionLookup::TopPadding;
        }
        if rel < -1 || rel > len {
            return SectionLookup::OutOfRange;
        }
        match self.sections[rel as usize] {
            Some(e) => SectionLookup::Loaded(e),
            None => SectionLookup::Unloaded,
        }
    }

    pub fn iter_wire(&self) -> impl Iterator<Item = SectionLookup> + '_ {
        std::iter::once(SectionLookup::BottomPadding)
            .chain(self.sections.iter().map(|slot| match slot {
                Some(e) => SectionLookup::Loaded(*e),
                None => SectionLookup::Unloaded,
            }))
            .chain(std::iter::once(SectionLookup::TopPadding))
    }

    pub fn set_loaded(&mut self, chunk_y: i32, entity: Entity) {
        let rel = chunk_y - self.min_section_y;
        if rel < 0 || (rel as usize) >= self.sections.len() {
            tracing::warn!(
                chunk_y,
                min_section_y = self.min_section_y,
                len = self.sections.len(),
                "set_loaded chunk_y out of range; ignored"
            );
            return;
        }
        self.sections[rel as usize] = Some(entity);
    }

    pub fn set_unloaded(&mut self, chunk_y: i32) {
        let rel = chunk_y - self.min_section_y;
        if rel < 0 || (rel as usize) >= self.sections.len() {
            tracing::warn!(
                chunk_y,
                min_section_y = self.min_section_y,
                len = self.sections.len(),
                "set_unloaded chunk_y out of range; ignored"
            );
            return;
        }
        self.sections[rel as usize] = None;
    }
}

impl Default for SectionIndex {
    fn default() -> Self {
        Self::new(0, 0)
    }
}

/// Bundle for chunk-column entities. Built via `ChunkColumnBundle::new` so the
/// `marker` field stays crate-private.
#[derive(Bundle)]
pub struct ChunkColumnBundle {
    pub col_pos: ChunkColumnPosComponent,
    pub dim: InDimension,
    pub heightmaps: Heightmaps,
    pub sections: SectionIndex,
    marker: ChunkColumn,
}

/// Component wrapper for `ChunkColumnPos` so it can live on the column entity.
#[derive(Component, Clone, Copy, Debug, Default, Deref, DerefMut)]
pub struct ChunkColumnPosComponent(pub ChunkColumnPos);

impl From<ChunkColumnPos> for ChunkColumnPosComponent {
    fn from(p: ChunkColumnPos) -> Self {
        Self(p)
    }
}

impl ChunkColumnBundle {
    pub fn new(col_pos: ChunkColumnPos, dim: InDimension, dim_config: &DimensionTypeConfig) -> Self {
        let min_section_y = dim_config.min_y.div_euclid(16);
        Self {
            col_pos: ChunkColumnPosComponent(col_pos),
            dim,
            heightmaps: Heightmaps::with_min_y(dim_config.height, dim_config.min_y),
            sections: SectionIndex::new(min_section_y, dim_config.section_count as usize),
            marker: ChunkColumn,
        }
    }
}

/// Ordered lifecycle stages for chunk-column reconciliation. Stages
/// `PrimeHeightmaps` and `AttachState` are reserved variants registered by
/// the lighting plugin (downstream); this plugin only registers
/// `Reconcile` and `ReconcileIndex`.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum ChunkColumnLifecycleSet {
    Reconcile,
    ReconcileIndex,
    PrimeHeightmaps,
    AttachState,
}

/// Stage 1: when a section becomes `ChunkLoaded` (or `ChunkUnloading`),
/// create / refcount its owning chunk-column entity.
fn reconcile_column_existence(
    newly_loaded: Query<(&ChunkPos, &InDimension), Added<ChunkLoaded>>,
    newly_unloading: Query<(&ChunkPos, &InDimension), Added<ChunkUnloading>>,
    mut dimensions: Query<&mut ColumnIndex>,
    dim_configs: Query<&DimensionTypeConfig>,
    mut commands: Commands,
) {
    for (chunk_pos, in_dim) in newly_loaded.iter() {
        let col_pos = ChunkColumnPos::from(*chunk_pos);
        let Ok(mut column_index) = dimensions.get_mut(in_dim.0) else {
            continue;
        };
        let Ok(dim_config) = dim_configs.get(in_dim.0) else {
            continue;
        };
        match column_index.0.entry(col_pos) {
            std::collections::hash_map::Entry::Vacant(v) => {
                let col_entity = commands
                    .spawn(ChunkColumnBundle::new(col_pos, *in_dim, dim_config))
                    .id();
                v.insert(ColumnSlot {
                    entity: col_entity,
                    section_count: 1,
                });
            }
            std::collections::hash_map::Entry::Occupied(mut o) => {
                o.get_mut().section_count += 1;
            }
        }
    }

    for (chunk_pos, in_dim) in newly_unloading.iter() {
        let col_pos = ChunkColumnPos::from(*chunk_pos);
        let Ok(mut column_index) = dimensions.get_mut(in_dim.0) else {
            continue;
        };
        let despawned = match column_index.0.get_mut(&col_pos) {
            Some(slot) => {
                if slot.section_count > 0 {
                    slot.section_count -= 1;
                } else {
                    tracing::warn!(
                        ?col_pos,
                        dim = ?in_dim.0,
                        "ChunkUnloading decrement past zero suppressed; refcount bug upstream"
                    );
                }
                if slot.section_count == 0 {
                    Some(slot.entity)
                } else {
                    None
                }
            }
            None => {
                tracing::warn!(
                    ?col_pos,
                    dim = ?in_dim.0,
                    "ChunkUnloading observed for section with no matching ColumnSlot entry"
                );
                None
            }
        };
        if let Some(entity) = despawned {
            commands.entity(entity).despawn();
            column_index.0.remove(&col_pos);
        }
    }
}

/// Stage 2: after Stage 1's `ApplyDeferred` flushes the spawn commands, the
/// new column entities are visible. Insert the section into its column's
/// `SectionIndex` and attach the `InChunkColumn` back-link.
///
/// Pitfall #1 safety check: this function does NOT take a lighting-table
/// resource. Heightmap priming (Stage 2.5) lives in `mcrs_lighting`.
fn reconcile_section_index(
    newly_loaded: Query<(Entity, &ChunkPos, &InDimension), Added<ChunkLoaded>>,
    newly_unloading: Query<(&ChunkPos, &InDimension), Added<ChunkUnloading>>,
    dimensions: Query<&ColumnIndex>,
    mut columns: Query<&mut SectionIndex>,
    mut commands: Commands,
) {
    for (section_entity, chunk_pos, in_dim) in newly_loaded.iter() {
        let col_pos = ChunkColumnPos::from(*chunk_pos);
        let Ok(column_index) = dimensions.get(in_dim.0) else {
            continue;
        };
        let Some(slot) = column_index.0.get(&col_pos) else {
            tracing::warn!(
                ?col_pos,
                dim = ?in_dim.0,
                "ChunkLoaded reached Stage 2 with no ColumnSlot — Stage 1 ApplyDeferred barrier failed"
            );
            continue;
        };
        if let Ok(mut section_index) = columns.get_mut(slot.entity) {
            section_index.set_loaded(chunk_pos.y, section_entity);
        }
        commands
            .entity(section_entity)
            .insert(InChunkColumn(slot.entity));
    }

    for (chunk_pos, in_dim) in newly_unloading.iter() {
        let col_pos = ChunkColumnPos::from(*chunk_pos);
        let Ok(column_index) = dimensions.get(in_dim.0) else {
            continue;
        };
        let Some(slot) = column_index.0.get(&col_pos) else {
            continue;
        };
        if let Ok(mut section_index) = columns.get_mut(slot.entity) {
            section_index.set_unloaded(chunk_pos.y);
        }
    }
}

pub struct ColumnPlugin;

impl Plugin for ColumnPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            FixedUpdate,
            (
                reconcile_column_existence.in_set(ChunkColumnLifecycleSet::Reconcile),
                ApplyDeferred,
                reconcile_section_index.in_set(ChunkColumnLifecycleSet::ReconcileIndex),
                // W6: trailing post-Stage-2 ApplyDeferred is intentionally omitted; the
                // lighting plugin owns the Stage 2 -> Stage 2.5 barrier with a leading
                // ApplyDeferred at the head of its own chain.
            )
                .chain(),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::entity::Entity;

    fn fake_entity(index: u32) -> Entity {
        Entity::from_raw_u32(index + 1).expect("valid entity index")
    }

    #[test]
    fn pbs_8bit_set_get_round_trip() {
        let mut pbs = PackedBitStorage::new(256, 0xFF);
        assert_eq!(pbs.bits_per_entry(), 8);
        for i in 0..256 {
            pbs.set(i, (i as u32) & 0xFF);
        }
        for i in 0..256 {
            assert_eq!(pbs.get(i), (i as u32) & 0xFF, "mismatch at {i}");
        }
    }

    #[test]
    fn pbs_9bit_specific_byte_layout() {
        // 9 bits per entry; entries_per_long = 64 / 9 = 7.
        let mut pbs = PackedBitStorage::with_bits(256, 9, 0x1FF);
        pbs.set(0, 0x1FF);
        pbs.set(1, 0x000);
        pbs.set(2, 0x1FF);
        let expected = (0x1FFu64 << 0) | (0x000u64 << 9) | (0x1FFu64 << 18);
        assert_eq!(
            pbs.raw_longs()[0],
            expected,
            "lowest entry must occupy lowest bits of long"
        );
        assert_eq!(pbs.get(0), 0x1FF);
        assert_eq!(pbs.get(1), 0x000);
        assert_eq!(pbs.get(2), 0x1FF);
    }

    #[test]
    #[should_panic(expected = "exceeds max_value")]
    fn pbs_value_clipping_debug_assert() {
        let mut pbs = PackedBitStorage::new(64, 0xFF);
        pbs.set(0, 0x100);
    }

    #[test]
    fn pbs_long_count_matches_ceil() {
        // 256 entries at 9 bits/entry: 7 entries per long -> ceil(256/7) = 37 longs.
        let pbs = PackedBitStorage::with_bits(256, 9, 0x1FF);
        assert_eq!(pbs.raw_longs().len(), 37);
    }

    #[test]
    fn pbs_bits_needed_for_boundaries() {
        assert_eq!(bits_needed_for(0), 1);
        assert_eq!(bits_needed_for(1), 1);
        assert_eq!(bits_needed_for(2), 2);
        assert_eq!(bits_needed_for(255), 8);
        assert_eq!(bits_needed_for(256), 9);
        assert_eq!(bits_needed_for(384), 9);
        assert_eq!(bits_needed_for(511), 9);
        assert_eq!(bits_needed_for(512), 10);
    }

    #[test]
    fn heightmap_new_dimensions_sized_correctly() {
        let h = Heightmaps::new(384);
        assert_eq!(h.world_surface.bits_per_entry(), 9);
        assert_eq!(h.world_surface.entry_count(), 256);
        // 256 entries / (64 / 9 = 7 per long) = 37 longs.
        assert_eq!(h.to_long_array_surface().len(), 37);
        assert_eq!(h.to_long_array_motion_blocking().len(), 37);
    }

    #[test]
    fn heightmap_set_get_round_trip() {
        let mut h = Heightmaps::new(384);
        for z in 0..16 {
            for x in 0..16 {
                let y = (z * 16 + x) as i32;
                h.surface_set(x, z, y);
            }
        }
        for z in 0..16 {
            for x in 0..16 {
                let y = (z * 16 + x) as i32;
                assert_eq!(h.surface_get(x, z), y, "surface mismatch at ({x}, {z})");
            }
        }
    }

    #[test]
    fn heightmap_to_long_array_vanilla_fixture() {
        // 9 bits per entry, lowest entry in lowest bits of long 0.
        let mut h = Heightmaps::new(384);
        // Index 0 = (x=0, z=0); index 1 = (x=1, z=0); index 2 = (x=2, z=0).
        h.surface_set(0, 0, 5); // value 5 at sub-position 0
        h.surface_set(1, 0, 10); // value 10 at sub-position 1
        h.surface_set(2, 0, 15); // value 15 at sub-position 2
        let expected = (5u64 << 0) | (10u64 << 9) | (15u64 << 18);
        assert_eq!(
            h.to_long_array_surface()[0],
            expected,
            "heightmap wire layout must match vanilla SimpleBitStorage"
        );
    }

    #[test]
    fn heightmap_zero_init_returns_min_y_for_unprimed_columns() {
        let h = Heightmaps::with_min_y(384, -64);
        assert_eq!(h.surface_get(0, 0), -64);
        assert_eq!(h.motion_blocking_get(15, 15), -64);
    }

    #[test]
    fn section_lookup_loaded() {
        let mut si = SectionIndex::new(-4, 24);
        let e = fake_entity(7);
        si.set_loaded(2, e);
        assert_eq!(si.lookup(2), SectionLookup::Loaded(e));
    }

    #[test]
    fn section_lookup_unloaded() {
        let si = SectionIndex::new(-4, 24);
        assert_eq!(si.lookup(0), SectionLookup::Unloaded);
    }

    #[test]
    fn section_lookup_bottom_padding() {
        let si = SectionIndex::new(-4, 24);
        assert_eq!(si.lookup(-5), SectionLookup::BottomPadding);
    }

    #[test]
    fn section_lookup_top_padding() {
        let si = SectionIndex::new(-4, 24);
        // min_section_y=-4, len=24 -> real range is -4..=19, top padding = 20.
        assert_eq!(si.lookup(20), SectionLookup::TopPadding);
    }

    #[test]
    fn section_lookup_out_of_range_low() {
        let si = SectionIndex::new(-4, 24);
        assert_eq!(si.lookup(-6), SectionLookup::OutOfRange);
    }

    #[test]
    fn section_lookup_out_of_range_high() {
        let si = SectionIndex::new(-4, 24);
        assert_eq!(si.lookup(21), SectionLookup::OutOfRange);
    }

    #[test]
    fn iter_wire_length_equals_real_plus_two() {
        let si = SectionIndex::new(-4, 24);
        assert_eq!(si.iter_wire().count(), 26);
    }

    #[test]
    fn iter_wire_first_is_bottom_padding() {
        let si = SectionIndex::new(-4, 24);
        let first = si.iter_wire().next().unwrap();
        assert_eq!(first, SectionLookup::BottomPadding);
    }

    #[test]
    fn iter_wire_last_is_top_padding() {
        let si = SectionIndex::new(-4, 24);
        let last = si.iter_wire().last().unwrap();
        assert_eq!(last, SectionLookup::TopPadding);
    }

    #[test]
    fn iter_wire_passes_loaded_and_unloaded() {
        let mut si = SectionIndex::new(0, 3);
        let e = fake_entity(11);
        si.set_loaded(1, e);
        let collected: Vec<_> = si.iter_wire().collect();
        assert_eq!(
            collected,
            vec![
                SectionLookup::BottomPadding,
                SectionLookup::Unloaded,
                SectionLookup::Loaded(e),
                SectionLookup::Unloaded,
                SectionLookup::TopPadding,
            ]
        );
    }

    #[test]
    fn column_bundle_constructor_uses_dim_config() {
        let dim_config = DimensionTypeConfig::new(-64, 384);
        let in_dim = InDimension(fake_entity(0));
        let col_pos = ChunkColumnPos::new(3, -5);
        let bundle = ChunkColumnBundle::new(col_pos, in_dim, &dim_config);
        assert_eq!(bundle.col_pos.0, col_pos);
        assert_eq!(bundle.sections.min_section_y, -4);
        assert_eq!(bundle.sections.sections.len(), 24);
        assert_eq!(bundle.heightmaps.height(), 384);
        assert_eq!(bundle.heightmaps.min_y(), -64);
    }

    #[test]
    fn column_bundle_nether_config() {
        // Nether: min_y=0, height=256, section_count=16. min_section_y=0.
        let dim_config = DimensionTypeConfig::new(0, 256);
        let in_dim = InDimension(fake_entity(0));
        let col_pos = ChunkColumnPos::new(0, 0);
        let bundle = ChunkColumnBundle::new(col_pos, in_dim, &dim_config);
        assert_eq!(bundle.sections.min_section_y, 0);
        assert_eq!(bundle.sections.sections.len(), 16);
        assert_eq!(bundle.heightmaps.height(), 256);
        assert_eq!(bundle.heightmaps.min_y(), 0);
    }

    #[test]
    fn column_slot_default_section_count() {
        let slot = ColumnSlot {
            entity: fake_entity(2),
            section_count: 1,
        };
        assert_eq!(slot.section_count, 1);
    }

    #[test]
    fn chunk_column_pos_from_chunk_pos_drops_y() {
        let cp = ChunkPos::new(3, 7, -5);
        let ccp: ChunkColumnPos = cp.into();
        assert_eq!(ccp, ChunkColumnPos::new(3, -5));
    }

    #[test]
    fn chunk_column_pos_from_block_pos_uses_div_euclid() {
        let bp = BlockPos::new(-1, 0, 17);
        let ccp: ChunkColumnPos = bp.into();
        assert_eq!(ccp, ChunkColumnPos::new(-1, 1));
    }
}
