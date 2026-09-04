// IMPORTANT: this module MUST NOT depend on the lighting crate. The four-stage
// ColumnLifecycleSet splits into storage-side (Reconcile, ReconcileIndex) and
// lighting-side (PrimeHeightmaps, AttachState) stages precisely because this
// crate sits upstream of lighting in the workspace graph.
//
// Heightmaps zero-init convention: `Heightmaps::new` zero-initializes the backing
// PackedBitStorage long arrays. `get(key, x, z) = min_y` for unprimed columns;
// downstream game code overwrites with real values before any consumer reads, and
// uses `min_y` as the "nothing found" sentinel.

use crate::world::dimension::{DimensionTypeConfig, InDimension};
use crate::world::lifecycle::markers::ChunkLoaded;
use crate::world::lifecycle::markers::ChunkUnloading;
use bevy_app::{App, FixedUpdate, Plugin};
use bevy_derive::{Deref, DerefMut};
use bevy_ecs::prelude::{
    Added, ApplyDeferred, Bundle, Commands, Component, Entity, IntoScheduleConfigs, Query, Res,
    Resource, SystemSet,
};
use mcrs_voxel_math::ChunkPos;
use mcrs_voxel_math::chunk_pos::BLOCKS;
use mcrs_voxel_storage::{PackedBitStorage, bits_needed_for};
use rustc_hash::FxHashMap;

pub use mcrs_voxel_math::ColumnPos;

/// Sparse marker component placed on chunk-column entities.
#[derive(Component, Debug, Default)]
#[component(storage = "SparseSet")]
pub struct Column;

/// Back-link from a chunk entity to its owning column entity.
/// Inserted by `reconcile_column_chunks` (Stage 2).
#[derive(Component, Clone, Copy, Debug)]
pub struct InColumn(pub Entity);

/// Per-column entry in `ColumnIndex`.
#[derive(Debug, Clone, Copy)]
pub struct ColumnSlot {
    pub entity: Entity,
    pub section_count: u32,
}

/// Per-dimension lookup from `ColumnPos` to the column entity + refcount.
/// Lives on the Dimension entity (added as a `DimensionBundle` field).
#[derive(Component, Debug, Default, Deref, DerefMut)]
pub struct ColumnIndex(pub FxHashMap<ColumnPos, ColumnSlot>);

/// Key into [`Heightmaps`], handed out by [`ColumnScalarRegistry::register`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ColumnScalarKey(pub usize);

/// The set of per-column scalars the game wants stored on every column.
/// Registration is idempotent, so a plugin may register its names without
/// caring whether a sibling plugin got there first.
#[derive(Resource, Debug, Clone, Default)]
pub struct ColumnScalarRegistry(Vec<String>);

impl ColumnScalarRegistry {
    pub fn register(&mut self, name: &str) -> ColumnScalarKey {
        if let Some(i) = self.0.iter().position(|n| n == name) {
            return ColumnScalarKey(i);
        }
        self.0.push(name.to_owned());
        ColumnScalarKey(self.0.len() - 1)
    }

    pub fn key(&self, name: &str) -> Option<ColumnScalarKey> {
        self.0.iter().position(|n| n == name).map(ColumnScalarKey)
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// One packed Y scalar per registered [`ColumnScalarKey`] over the 16x16
/// column footprint. Indexed by `(x, z)` in `0..16` each; the entry index is
/// `z * BLOCKS::SIZE + x`. Stored Y values are absolute world Y.
#[derive(Component, Debug, Clone)]
pub struct Heightmaps {
    stores: Box<[PackedBitStorage]>,
    height: u32,
    min_y: i32,
}

impl Heightmaps {
    /// Create heightmaps sized to the dimension height. `min_y` defaults to 0;
    /// use `with_min_y` for dimensions whose lowest section is negative.
    pub fn new(scalar_count: usize, height: u32) -> Self {
        Self::with_min_y(scalar_count, height, 0)
    }

    pub fn with_min_y(scalar_count: usize, height: u32, min_y: i32) -> Self {
        let max_value = height; // stored value range is [0, height]
        let bits = bits_needed_for(max_value);
        Self {
            stores: (0..scalar_count)
                .map(|_| PackedBitStorage::with_bits(BLOCKS::AREA, bits, max_value))
                .collect(),
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

    pub fn scalar_count(&self) -> usize {
        self.stores.len()
    }

    #[inline]
    fn index(x: usize, z: usize) -> usize {
        debug_assert!(
            x < BLOCKS::SIZE && z < BLOCKS::SIZE,
            "Heightmaps index ({x}, {z}) out of range"
        );
        (z & BLOCKS::MASK) * BLOCKS::SIZE + (x & BLOCKS::MASK)
    }

    /// Panics rather than indexing blindly: a column sized before the scalar
    /// was registered would otherwise fail far from the plugin that skipped it.
    #[inline]
    #[track_caller]
    fn slot(stores: usize, key: ColumnScalarKey) -> usize {
        assert!(
            key.0 < stores,
            "ColumnScalarKey({}) was not registered before this column was spawned (scalar_count={stores})",
            key.0,
        );
        key.0
    }

    #[inline]
    fn store(&self, key: ColumnScalarKey) -> &PackedBitStorage {
        &self.stores[Self::slot(self.stores.len(), key)]
    }

    pub fn get(&self, key: ColumnScalarKey, x: usize, z: usize) -> i32 {
        self.store(key).get(Self::index(x, z)) as i32 + self.min_y
    }

    pub fn set(&mut self, key: ColumnScalarKey, x: usize, z: usize, y: i32) {
        let max_stored = self.min_y + self.height as i32;
        debug_assert!(
            y >= self.min_y && y <= max_stored,
            "Heightmaps::set y={y} outside [{min}, {max}]",
            min = self.min_y,
            max = max_stored,
        );
        let rel = (y - self.min_y).clamp(0, self.height as i32);
        let index = Self::index(x, z);
        let slot = Self::slot(self.stores.len(), key);
        self.stores[slot].set(index, rel as u32);
    }

    pub fn raw_longs(&self, key: ColumnScalarKey) -> &[u64] {
        self.store(key).raw_longs()
    }

    pub fn storage(&self, key: ColumnScalarKey) -> &PackedBitStorage {
        self.store(key)
    }
}

// No `Default for Heightmaps`: every column ships with a dimension-shape
// derived size via `Heightmaps::with_min_y`, and any hardcoded default height
// would silently mis-size storage for every dimension of a different height.
// Callers that need a fresh heightmap must go through
// `DimensionTypeConfig` so the right shape is plumbed in.

/// Result of looking up a chunk by `chunk_y` inside a column.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChunkLookup {
    Loaded(Entity),
    Unloaded,
    OutOfRange,
}

/// Per-column index of real chunk entities.
#[derive(Component, Debug, Clone)]
pub struct ColumnChunks {
    pub min_section_y: i32,
    pub sections: Box<[Option<Entity>]>,
}

impl ColumnChunks {
    pub fn new(min_section_y: i32, real_count: usize) -> Self {
        Self {
            min_section_y,
            sections: vec![None; real_count].into_boxed_slice(),
        }
    }

    pub fn lookup(&self, chunk_y: i32) -> ChunkLookup {
        let rel = chunk_y - self.min_section_y;
        if rel < 0 || rel as usize >= self.sections.len() {
            return ChunkLookup::OutOfRange;
        }
        match self.sections[rel as usize] {
            Some(e) => ChunkLookup::Loaded(e),
            None => ChunkLookup::Unloaded,
        }
    }

    /// Every real section in ascending Y, starting at `min_section_y`.
    pub fn iter(&self) -> impl Iterator<Item = ChunkLookup> + '_ {
        self.sections.iter().map(|slot| match slot {
            Some(e) => ChunkLookup::Loaded(*e),
            None => ChunkLookup::Unloaded,
        })
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

impl Default for ColumnChunks {
    fn default() -> Self {
        Self::new(0, 0)
    }
}

/// Bundle for chunk-column entities. Built via `ColumnBundle::new` so the
/// `marker` field stays crate-private.
#[derive(Bundle)]
pub struct ColumnBundle {
    pub col_pos: ColumnPosComponent,
    pub dim: InDimension,
    pub heightmaps: Heightmaps,
    pub sections: ColumnChunks,
    marker: Column,
}

/// Component wrapper for `ColumnPos` so it can live on the column entity.
#[derive(Component, Clone, Copy, Debug, Default, Deref, DerefMut)]
pub struct ColumnPosComponent(pub ColumnPos);

impl From<ColumnPos> for ColumnPosComponent {
    fn from(p: ColumnPos) -> Self {
        Self(p)
    }
}

impl ColumnBundle {
    pub fn new(
        col_pos: ColumnPos,
        dim: InDimension,
        dim_config: &DimensionTypeConfig,
        scalars: &ColumnScalarRegistry,
    ) -> Self {
        let min_section_y = dim_config.min_y >> BLOCKS::BITS;
        Self {
            col_pos: ColumnPosComponent(col_pos),
            dim,
            heightmaps: Heightmaps::with_min_y(scalars.len(), dim_config.height, dim_config.min_y),
            sections: ColumnChunks::new(min_section_y, dim_config.section_count as usize),
            marker: Column,
        }
    }
}

/// Ordered lifecycle stages for chunk-column reconciliation. Stages
/// `PrimeHeightmaps` and `AttachState` are reserved variants registered by
/// the lighting plugin (downstream); this plugin only registers
/// `Reconcile` and `ReconcileIndex`.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum ColumnLifecycleSet {
    Reconcile,
    ReconcileIndex,
    PrimeHeightmaps,
    AttachState,
}

/// Stage 1: when a chunk becomes `ChunkLoaded` (or `ChunkUnloading`),
/// create / refcount its owning column entity.
pub fn reconcile_column_existence(
    newly_loaded: Query<(&ChunkPos, &InDimension), Added<ChunkLoaded>>,
    newly_unloading: Query<(&ChunkPos, &InDimension), Added<ChunkUnloading>>,
    mut dimensions: Query<&mut ColumnIndex>,
    dim_configs: Query<&DimensionTypeConfig>,
    scalars: Res<ColumnScalarRegistry>,
    mut commands: Commands,
) {
    for (chunk_pos, in_dim) in newly_loaded.iter() {
        let col_pos = ColumnPos::from(*chunk_pos);
        let Ok(mut column_index) = dimensions.get_mut(in_dim.0) else {
            continue;
        };
        let Ok(dim_config) = dim_configs.get(in_dim.0) else {
            continue;
        };
        match column_index.0.entry(col_pos) {
            std::collections::hash_map::Entry::Vacant(v) => {
                let col_entity = commands
                    .spawn(ColumnBundle::new(col_pos, *in_dim, dim_config, &scalars))
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
        let col_pos = ColumnPos::from(*chunk_pos);
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
                    "ChunkUnloading observed for chunk with no matching ColumnSlot entry"
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
/// new column entities are visible. Insert the chunk into its column's
/// `ColumnChunks` and attach the `InColumn` back-link.
///
/// Deliberately takes no lighting-table resource: heightmap priming
/// (Stage 2.5) lives in the lighting crate.
pub fn reconcile_column_chunks(
    newly_loaded: Query<(Entity, &ChunkPos, &InDimension), Added<ChunkLoaded>>,
    newly_unloading: Query<(&ChunkPos, &InDimension), Added<ChunkUnloading>>,
    dimensions: Query<&ColumnIndex>,
    mut columns: Query<&mut ColumnChunks>,
    mut commands: Commands,
) {
    for (chunk_entity, chunk_pos, in_dim) in newly_loaded.iter() {
        let col_pos = ColumnPos::from(*chunk_pos);
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
        if let Ok(mut column_chunks) = columns.get_mut(slot.entity) {
            column_chunks.set_loaded(chunk_pos.y, chunk_entity);
        }
        commands.entity(chunk_entity).insert(InColumn(slot.entity));
    }

    for (chunk_pos, in_dim) in newly_unloading.iter() {
        let col_pos = ColumnPos::from(*chunk_pos);
        let Ok(column_index) = dimensions.get(in_dim.0) else {
            continue;
        };
        let Some(slot) = column_index.0.get(&col_pos) else {
            continue;
        };
        if let Ok(mut column_chunks) = columns.get_mut(slot.entity) {
            column_chunks.set_unloaded(chunk_pos.y);
        }
    }
}

pub struct ColumnPlugin;

impl Plugin for ColumnPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ColumnScalarRegistry>();
        app.add_systems(
            FixedUpdate,
            (
                reconcile_column_existence.in_set(ColumnLifecycleSet::Reconcile),
                ApplyDeferred,
                reconcile_column_chunks.in_set(ColumnLifecycleSet::ReconcileIndex),
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
    use mcrs_voxel_math::BlockPos;

    fn fake_entity(index: u32) -> Entity {
        Entity::from_raw_u32(index + 1).expect("valid entity index")
    }

    #[test]
    fn heightmap_new_dimensions_sized_correctly() {
        let h = Heightmaps::new(2, 384);
        assert_eq!(h.storage(ColumnScalarKey(0)).bits_per_entry(), 9);
        assert_eq!(h.storage(ColumnScalarKey(0)).entry_count(), 256);
        // 256 entries / (64 / 9 = 7 per long) = 37 longs.
        assert_eq!(h.raw_longs(ColumnScalarKey(0)).len(), 37);
        assert_eq!(h.raw_longs(ColumnScalarKey(1)).len(), 37);
    }

    #[test]
    fn heightmap_set_get_round_trip() {
        let mut h = Heightmaps::new(2, 384);
        for z in 0..BLOCKS::SIZE {
            for x in 0..BLOCKS::SIZE {
                let y = (z * BLOCKS::SIZE + x) as i32;
                h.set(ColumnScalarKey(0), x, z, y);
            }
        }
        for z in 0..BLOCKS::SIZE {
            for x in 0..BLOCKS::SIZE {
                let y = (z * BLOCKS::SIZE + x) as i32;
                assert_eq!(
                    h.get(ColumnScalarKey(0), x, z),
                    y,
                    "scalar mismatch at ({x}, {z})"
                );
            }
        }
    }

    #[test]
    fn heightmap_packs_entries_lowest_index_in_lowest_bits() {
        // 9 bits per entry, lowest entry in lowest bits of long 0.
        let mut h = Heightmaps::new(2, 384);
        // Index 0 = (x=0, z=0); index 1 = (x=1, z=0); index 2 = (x=2, z=0).
        h.set(ColumnScalarKey(0), 0, 0, 5); // value 5 at sub-position 0
        h.set(ColumnScalarKey(0), 1, 0, 10); // value 10 at sub-position 1
        h.set(ColumnScalarKey(0), 2, 0, 15); // value 15 at sub-position 2
        let expected = 5u64 | (10u64 << 9) | (15u64 << 18);
        assert_eq!(
            h.raw_longs(ColumnScalarKey(0))[0],
            expected,
            "entry n must occupy bits [n*bits, (n+1)*bits) of the long array"
        );
    }

    #[test]
    fn heightmap_zero_init_returns_min_y_for_unprimed_columns() {
        let h = Heightmaps::with_min_y(2, 384, -64);
        assert_eq!(h.get(ColumnScalarKey(0), 0, 0), -64);
        assert_eq!(h.get(ColumnScalarKey(1), BLOCKS::MASK, BLOCKS::MASK), -64);
    }

    #[test]
    fn section_lookup_loaded() {
        let mut si = ColumnChunks::new(-4, 24);
        let e = fake_entity(7);
        si.set_loaded(2, e);
        assert_eq!(si.lookup(2), ChunkLookup::Loaded(e));
    }

    #[test]
    fn section_lookup_unloaded() {
        let si = ColumnChunks::new(-4, 24);
        assert_eq!(si.lookup(0), ChunkLookup::Unloaded);
    }

    #[test]
    fn section_lookup_just_below_range() {
        let si = ColumnChunks::new(-4, 24);
        assert_eq!(si.lookup(-5), ChunkLookup::OutOfRange);
    }

    #[test]
    fn section_lookup_just_above_range() {
        let si = ColumnChunks::new(-4, 24);
        // min_section_y=-4, len=24 -> real range is -4..=19.
        assert_eq!(si.lookup(20), ChunkLookup::OutOfRange);
    }

    #[test]
    fn section_lookup_out_of_range_low() {
        let si = ColumnChunks::new(-4, 24);
        assert_eq!(si.lookup(-6), ChunkLookup::OutOfRange);
    }

    #[test]
    fn section_lookup_out_of_range_high() {
        let si = ColumnChunks::new(-4, 24);
        assert_eq!(si.lookup(21), ChunkLookup::OutOfRange);
    }

    #[test]
    fn iter_length_equals_real_count() {
        let si = ColumnChunks::new(-4, 24);
        assert_eq!(si.iter().count(), 24);
    }

    #[test]
    fn iter_passes_loaded_and_unloaded() {
        let mut si = ColumnChunks::new(0, 3);
        let e = fake_entity(11);
        si.set_loaded(1, e);
        let collected: Vec<_> = si.iter().collect();
        assert_eq!(
            collected,
            vec![
                ChunkLookup::Unloaded,
                ChunkLookup::Loaded(e),
                ChunkLookup::Unloaded,
            ]
        );
    }

    #[test]
    fn column_scalar_registry_is_idempotent() {
        let mut r = ColumnScalarRegistry::default();
        let a = r.register("a");
        let b = r.register("b");
        assert_eq!(r.register("a"), a);
        assert_eq!(b, ColumnScalarKey(1));
        assert_eq!(r.len(), 2);
        assert_eq!(r.key("b"), Some(b));
        assert_eq!(r.key("c"), None);
    }

    #[test]
    fn column_bundle_constructor_uses_dim_config() {
        let dim_config = DimensionTypeConfig::new(-64, 384);
        let in_dim = InDimension(fake_entity(0));
        let col_pos = ColumnPos::new(3, -5);
        let mut scalars = ColumnScalarRegistry::default();
        scalars.register("a");
        let bundle = ColumnBundle::new(col_pos, in_dim, &dim_config, &scalars);
        assert_eq!(bundle.heightmaps.scalar_count(), 1);
        assert_eq!(bundle.col_pos.0, col_pos);
        assert_eq!(bundle.sections.min_section_y, -4);
        assert_eq!(bundle.sections.sections.len(), 24);
        assert_eq!(bundle.heightmaps.height(), 384);
        assert_eq!(bundle.heightmaps.min_y(), -64);
    }

    #[test]
    fn column_bundle_with_non_negative_min_y() {
        let dim_config = DimensionTypeConfig::new(0, 256);
        let in_dim = InDimension(fake_entity(0));
        let col_pos = ColumnPos::new(0, 0);
        let scalars = ColumnScalarRegistry::default();
        let bundle = ColumnBundle::new(col_pos, in_dim, &dim_config, &scalars);
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
        let ccp: ColumnPos = cp.into();
        assert_eq!(ccp, ColumnPos::new(3, -5));
    }

    #[test]
    fn chunk_column_pos_from_block_pos_uses_div_euclid() {
        let bp = BlockPos::new(-1, 0, 17);
        let ccp: ColumnPos = bp.into();
        assert_eq!(ccp, ColumnPos::new(-1, 1));
    }
}
