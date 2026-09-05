// The ColumnLifecycleSet stages exist so downstream crates can order their own
// per-column work against the storage-side reconciliation without this crate
// having to know about them.

use crate::world::dimension::{DimensionTypeConfig, InDimension};
use crate::world::lifecycle::markers::{ChunkFresh, ChunkLoaded, ChunkUnloading};
use bevy_app::{App, FixedUpdate, Plugin};
use bevy_derive::{Deref, DerefMut};
use bevy_ecs::prelude::{
    Added, ApplyDeferred, Bundle, Commands, Component, Entity, IntoScheduleConfigs, Query,
    SystemSet, With,
};
use mcrs_voxel_math::ChunkPos;
use mcrs_voxel_math::chunk_pos::BLOCKS;
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
    pub fn new(col_pos: ColumnPos, dim: InDimension, dim_config: &DimensionTypeConfig) -> Self {
        let min_section_y = dim_config.min_y >> BLOCKS::BITS;
        Self {
            col_pos: ColumnPosComponent(col_pos),
            dim,
            sections: ColumnChunks::new(min_section_y, dim_config.section_count as usize),
            marker: Column,
        }
    }
}

/// Ordered lifecycle stages for chunk-column reconciliation.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum ColumnLifecycleSet {
    Reconcile,
    ReconcileIndex,
}

/// Stage 1: when a chunk becomes `ChunkLoaded` (or `ChunkUnloading`),
/// create / refcount its owning column entity.
///
/// Only a section Stage 2 gave an `InColumn` was ever counted, so only such a
/// section may decrement: one cancelled before it loaded reaches `ChunkUnloading`
/// having never joined a column.
pub fn reconcile_column_existence(
    newly_loaded: Query<(&ChunkPos, &InDimension), (Added<ChunkLoaded>, With<ChunkFresh>)>,
    newly_unloading: Query<(&ChunkPos, &InDimension), (Added<ChunkUnloading>, With<InColumn>)>,
    mut dimensions: Query<&mut ColumnIndex>,
    dim_configs: Query<&DimensionTypeConfig>,
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
                    .spawn(ColumnBundle::new(col_pos, *in_dim, dim_config))
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
pub fn reconcile_column_chunks(
    newly_loaded: Query<(Entity, &ChunkPos, &InDimension), (Added<ChunkLoaded>, With<ChunkFresh>)>,
    newly_unloading: Query<(&ChunkPos, &InDimension), (Added<ChunkUnloading>, With<InColumn>)>,
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
        app.add_systems(
            FixedUpdate,
            (
                reconcile_column_existence.in_set(ColumnLifecycleSet::Reconcile),
                ApplyDeferred,
                reconcile_column_chunks.in_set(ColumnLifecycleSet::ReconcileIndex),
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
    fn a_section_cancelled_before_it_loaded_does_not_decrement_its_column() {
        let mut app = App::new();
        app.add_systems(
            FixedUpdate,
            (
                reconcile_column_existence,
                ApplyDeferred,
                reconcile_column_chunks,
            )
                .chain(),
        );
        let dim = app
            .world_mut()
            .spawn((ColumnIndex::default(), DimensionTypeConfig::new(0, 256)))
            .id();
        let pos = ChunkPos::new(0, 0, 0);
        app.world_mut().spawn((pos, InDimension(dim), ChunkLoaded));
        app.world_mut().run_schedule(FixedUpdate);

        app.world_mut()
            .spawn((ChunkPos::new(0, 1, 0), InDimension(dim), ChunkUnloading));
        app.world_mut().run_schedule(FixedUpdate);

        let index = app.world().get::<ColumnIndex>(dim).expect("column index");
        assert_eq!(
            index.0.get(&ColumnPos::new(0, 0)).map(|s| s.section_count),
            Some(1),
        );
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
    fn column_bundle_constructor_uses_dim_config() {
        let dim_config = DimensionTypeConfig::new(-64, 384);
        let in_dim = InDimension(fake_entity(0));
        let col_pos = ColumnPos::new(3, -5);
        let bundle = ColumnBundle::new(col_pos, in_dim, &dim_config);
        assert_eq!(bundle.col_pos.0, col_pos);
        assert_eq!(bundle.sections.min_section_y, -4);
        assert_eq!(bundle.sections.sections.len(), 24);
    }

    #[test]
    fn column_bundle_with_non_negative_min_y() {
        let dim_config = DimensionTypeConfig::new(0, 256);
        let in_dim = InDimension(fake_entity(0));
        let col_pos = ColumnPos::new(0, 0);
        let bundle = ColumnBundle::new(col_pos, in_dim, &dim_config);
        assert_eq!(bundle.sections.min_section_y, 0);
        assert_eq!(bundle.sections.sections.len(), 16);
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
