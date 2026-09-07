use crate::world::dimension::{DimensionTypeConfig, InDimension};
use crate::world::lifecycle::markers::{ChunkFresh, ChunkLoaded, ChunkUnloaded, ChunkUnloading};
use bevy_app::{App, FixedUpdate, Plugin};
use bevy_derive::{Deref, DerefMut};
use bevy_ecs::prelude::{
    Added, Bundle, Commands, Component, Entity, IntoScheduleConfigs, Query, SystemSet, With,
    Without,
};
use mcrs_voxel_math::ChunkPos;
use mcrs_voxel_math::chunk_pos::BLOCKS;
use rustc_hash::FxHashMap;

pub use mcrs_voxel_math::ColumnPos;

/// Sparse marker component placed on chunk-column entities.
#[derive(Component, Debug, Default)]
#[component(storage = "SparseSet")]
pub struct Column;

/// Back-link from a chunk entity to its owning column entity. It is what says a
/// section is counted in its column's `section_count`: `reconcile_columns`
/// attaches it as the section joins and takes it off as the section leaves, in
/// the same run as the matching increment or decrement.
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

/// Anchor for downstream per-column work to order itself against.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct ColumnLifecycleSet;

/// Reconciles the column index against the sections that landed or left this
/// run. One system rather than two: a split pair has one `Added` window per
/// system, and the two instances of the pair (the tick and the column drain)
/// interleave, so a section could be counted by one half and not the other.
///
/// A section already on its way out is not counted at all. Cancelling one whose
/// generation had finished leaves it holding `ChunkLoaded` and `ChunkUnloading`
/// together, and counting it would spend an unloading edge that has already
/// gone by — the column would then hold a section that never leaves it.
pub fn reconcile_columns(
    newly_loaded: Query<
        (Entity, &ChunkPos, &InDimension),
        (
            Added<ChunkLoaded>,
            With<ChunkFresh>,
            Without<InColumn>,
            Without<ChunkUnloading>,
            Without<ChunkUnloaded>,
        ),
    >,
    newly_unloading: Query<
        (Entity, &ChunkPos, &InDimension),
        (Added<ChunkUnloading>, With<InColumn>),
    >,
    mut dimensions: Query<&mut ColumnIndex>,
    dim_configs: Query<&DimensionTypeConfig>,
    mut columns: Query<&mut ColumnChunks>,
    mut commands: Commands,
) {
    // A column spawned this run is not in `columns` yet, so its sections are
    // gathered here and land with the entity's own `ColumnChunks`.
    let mut spawned: FxHashMap<Entity, ColumnChunks> = FxHashMap::default();

    for (chunk_entity, chunk_pos, in_dim) in newly_loaded.iter() {
        let col_pos = ColumnPos::from(*chunk_pos);
        let Ok(mut column_index) = dimensions.get_mut(in_dim.0) else {
            continue;
        };
        let Ok(dim_config) = dim_configs.get(in_dim.0) else {
            continue;
        };
        let slot = *match column_index.0.entry(col_pos) {
            std::collections::hash_map::Entry::Vacant(v) => {
                let col_entity = commands
                    .spawn(ColumnBundle::new(col_pos, *in_dim, dim_config))
                    .id();
                spawned.insert(
                    col_entity,
                    ColumnChunks::new(
                        dim_config.min_y >> BLOCKS::BITS,
                        dim_config.section_count as usize,
                    ),
                );
                v.insert(ColumnSlot {
                    entity: col_entity,
                    section_count: 1,
                })
            }
            std::collections::hash_map::Entry::Occupied(o) => {
                let slot = o.into_mut();
                slot.section_count += 1;
                slot
            }
        };
        match spawned.get_mut(&slot.entity) {
            Some(column_chunks) => column_chunks.set_loaded(chunk_pos.y, chunk_entity),
            None => columns
                .get_mut(slot.entity)
                .unwrap_or_else(|_| {
                    panic!("column {col_pos:?} is in the index without its ColumnChunks")
                })
                .set_loaded(chunk_pos.y, chunk_entity),
        }
        commands.entity(chunk_entity).insert(InColumn(slot.entity));
    }

    for (col_entity, column_chunks) in spawned {
        commands.entity(col_entity).insert(column_chunks);
    }

    for (chunk_entity, chunk_pos, in_dim) in newly_unloading.iter() {
        commands.entity(chunk_entity).try_remove::<InColumn>();
        let col_pos = ColumnPos::from(*chunk_pos);
        let Ok(mut column_index) = dimensions.get_mut(in_dim.0) else {
            continue;
        };
        let slot = column_index.0.get_mut(&col_pos).unwrap_or_else(|| {
            panic!("section of {col_pos:?} holds InColumn but the column left the index")
        });
        assert!(
            slot.section_count > 0,
            "column {col_pos:?} counts fewer sections than hold its InColumn",
        );
        slot.section_count -= 1;
        let column_entity = slot.entity;
        let emptied = slot.section_count == 0;
        if let Ok(mut column_chunks) = columns.get_mut(column_entity) {
            column_chunks.set_unloaded(chunk_pos.y);
        }
        if emptied {
            column_index.0.remove(&col_pos);
            commands.entity(column_entity).despawn();
        }
    }
}

pub struct ColumnPlugin;

impl Plugin for ColumnPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(FixedUpdate, reconcile_columns.in_set(ColumnLifecycleSet));
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
        app.add_systems(FixedUpdate, reconcile_columns);
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
    fn a_section_unloaded_twice_only_decrements_once() {
        let mut app = App::new();
        app.add_systems(FixedUpdate, reconcile_columns);
        let dim = app
            .world_mut()
            .spawn((ColumnIndex::default(), DimensionTypeConfig::new(0, 256)))
            .id();
        let kept = app
            .world_mut()
            .spawn((ChunkPos::new(0, 0, 0), InDimension(dim), ChunkLoaded))
            .id();
        let leaving = app
            .world_mut()
            .spawn((ChunkPos::new(0, 1, 0), InDimension(dim), ChunkLoaded))
            .id();
        app.world_mut().run_schedule(FixedUpdate);

        let count = |app: &App| {
            app.world()
                .get::<ColumnIndex>(dim)
                .expect("column index")
                .0
                .get(&ColumnPos::new(0, 0))
                .map(|s| s.section_count)
        };
        assert_eq!(count(&app), Some(2));

        app.world_mut().entity_mut(leaving).insert(ChunkUnloading);
        app.world_mut().run_schedule(FixedUpdate);
        assert_eq!(count(&app), Some(1));

        // A chunk waiting to despawn can be handed tickets and lose them again,
        // so the same section reaches `ChunkUnloading` a second time.
        app.world_mut()
            .entity_mut(leaving)
            .remove::<ChunkUnloading>();
        app.world_mut().run_schedule(FixedUpdate);
        app.world_mut().entity_mut(leaving).insert(ChunkUnloading);
        app.world_mut().run_schedule(FixedUpdate);
        assert_eq!(count(&app), Some(1));

        assert!(app.world().get::<InColumn>(kept).is_some());
    }

    /// A section cancelled after its generation finished carries `ChunkUnloading`
    /// and `ChunkLoaded` at once, and the tick's reconciler and the drain's see
    /// that pair through separate `Added` windows.
    #[test]
    fn a_section_that_lands_already_cancelled_leaves_no_column_behind() {
        #[derive(bevy_ecs::schedule::ScheduleLabel, Debug, Clone, PartialEq, Eq, Hash)]
        struct Drain;

        let mut app = App::new();
        app.add_systems(FixedUpdate, reconcile_columns);
        app.add_systems(Drain, reconcile_columns);
        let dim = app
            .world_mut()
            .spawn((ColumnIndex::default(), DimensionTypeConfig::new(0, 256)))
            .id();

        let chunk = app
            .world_mut()
            .spawn((ChunkPos::new(0, 0, 0), InDimension(dim), ChunkUnloading))
            .id();
        app.world_mut().run_schedule(FixedUpdate);
        app.world_mut().entity_mut(chunk).insert(ChunkLoaded);
        app.world_mut().run_schedule(Drain);
        app.world_mut().run_schedule(FixedUpdate);
        app.world_mut().run_schedule(Drain);

        let index = app.world().get::<ColumnIndex>(dim).expect("column index");
        assert_eq!(
            index.0.get(&ColumnPos::new(0, 0)).map(|s| s.section_count),
            None
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
