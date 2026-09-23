use crate::aoi::PlayerObservers;
use crate::world::dimension::{DimensionTypeConfig, InDimension};
use crate::world::lifecycle::stage::{SectionStage, SectionStageChanged};
use bevy_app::{App, FixedUpdate, Plugin};
use bevy_derive::{Deref, DerefMut};
use bevy_ecs::prelude::{
    Bundle, Commands, Component, Entity, IntoScheduleConfigs, MessageReader, Query, SystemSet,
};
use mcrs_minecraft_core::SectionPos;
use rustc_hash::FxHashMap;

pub use mcrs_minecraft_core::ColumnPos;

/// Sparse marker component placed on chunk-column entities.
#[derive(Component, Debug, Default)]
#[component(storage = "SparseSet")]
pub struct Column;

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
pub enum SectionLookup {
    Loaded(Entity),
    Unloaded,
    OutOfRange,
}

/// Per-column index of real chunk entities.
#[derive(Component, Debug, Clone)]
pub struct ColumnSections {
    pub min_section_y: i32,
    pub sections: Box<[Option<Entity>]>,
}

impl ColumnSections {
    pub fn new(min_section_y: i32, real_count: usize) -> Self {
        Self {
            min_section_y,
            sections: vec![None; real_count].into_boxed_slice(),
        }
    }

    pub fn lookup(&self, chunk_y: i32) -> SectionLookup {
        let rel = chunk_y - self.min_section_y;
        if rel < 0 || rel as usize >= self.sections.len() {
            return SectionLookup::OutOfRange;
        }
        match self.sections[rel as usize] {
            Some(e) => SectionLookup::Loaded(e),
            None => SectionLookup::Unloaded,
        }
    }

    /// Every real section in ascending Y, starting at `min_section_y`.
    pub fn iter(&self) -> impl Iterator<Item = SectionLookup> + '_ {
        self.sections.iter().map(|slot| match slot {
            Some(e) => SectionLookup::Loaded(*e),
            None => SectionLookup::Unloaded,
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

impl Default for ColumnSections {
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
    pub sections: ColumnSections,
    pub observers: PlayerObservers,
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
        let min_section_y = dim_config.min_y >> SectionPos::BITS;
        Self {
            col_pos: ColumnPosComponent(col_pos),
            dim,
            sections: ColumnSections::new(min_section_y, dim_config.section_count as usize),
            observers: PlayerObservers::default(),
            marker: Column,
        }
    }
}

/// Anchor for downstream per-column work to order itself against.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct ColumnLifecycleSet;

/// Brings the column index in line with every section whose stage crossed
/// `Loaded`. It decides from the stage the section holds now rather than from
/// the edge, so the tick's run and the drain's run can both read the same
/// messages and the second finds nothing left to do.
pub fn reconcile_columns(
    mut changes: MessageReader<SectionStageChanged>,
    stages: Query<&SectionStage>,
    mut dimensions: Query<(&mut ColumnIndex, &DimensionTypeConfig)>,
    mut columns: Query<&mut ColumnSections>,
    mut commands: Commands,
) {
    // A column spawned this run is not in `columns` yet, so its sections are
    // gathered here and land with the entity's own `ColumnSections`.
    let mut spawned: FxHashMap<Entity, ColumnSections> = FxHashMap::default();

    for change in changes.read() {
        if !change.landed() && !change.left() {
            continue;
        }
        let Ok((mut column_index, dim_config)) = dimensions.get_mut(change.dim) else {
            continue;
        };
        let col_pos = ColumnPos::from(change.pos);
        let y = change.pos.y;
        let loaded = stages.get(change.section) == Ok(&SectionStage::Loaded);
        let holds =
            |sections: &ColumnSections| sections.lookup(y) == SectionLookup::Loaded(change.section);
        let counted =
            column_index
                .0
                .get(&col_pos)
                .is_some_and(|slot| match spawned.get(&slot.entity) {
                    Some(sections) => holds(sections),
                    None => columns.get(slot.entity).is_ok_and(holds),
                });

        if loaded && !counted {
            let slot = column_index.0.entry(col_pos).or_insert_with(|| {
                let entity = commands
                    .spawn(ColumnBundle::new(
                        col_pos,
                        InDimension(change.dim),
                        dim_config,
                    ))
                    .id();
                spawned.insert(
                    entity,
                    ColumnSections::new(
                        dim_config.min_y >> SectionPos::BITS,
                        dim_config.section_count as usize,
                    ),
                );
                ColumnSlot {
                    entity,
                    section_count: 0,
                }
            });
            let sections = match spawned.get_mut(&slot.entity) {
                Some(sections) => sections,
                None => columns
                    .get_mut(slot.entity)
                    .unwrap_or_else(|_| {
                        panic!("column {col_pos:?} is in the index without its ColumnSections")
                    })
                    .into_inner(),
            };
            if sections.lookup(y) == SectionLookup::Unloaded {
                slot.section_count += 1;
            }
            sections.set_loaded(y, change.section);
        } else if !loaded && counted {
            let slot = column_index
                .0
                .get_mut(&col_pos)
                .expect("a counted section's column is in the index");
            assert!(
                slot.section_count > 0,
                "column {col_pos:?} counts fewer sections than it holds",
            );
            slot.section_count -= 1;
            let column_entity = slot.entity;
            let emptied = slot.section_count == 0;
            if let Ok(mut sections) = columns.get_mut(column_entity) {
                sections.set_unloaded(y);
            }
            if emptied {
                column_index.0.remove(&col_pos);
                commands.entity(column_entity).despawn();
            }
        }
    }

    for (column_entity, sections) in spawned {
        commands.entity(column_entity).insert(sections);
    }
}

pub struct ColumnPlugin;

impl Plugin for ColumnPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            FixedUpdate,
            (
                reconcile_columns,
                crate::world::storage::block_entity::reconcile_block_entities,
            )
                .chain()
                .in_set(ColumnLifecycleSet),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::entity::Entity;
    use mcrs_minecraft_core::BlockPos;

    fn fake_entity(index: u32) -> Entity {
        Entity::from_raw_u32(index + 1).expect("valid entity index")
    }

    fn app_with_dimension() -> (App, Entity) {
        let mut app = App::new();
        app.add_message::<SectionStageChanged>();
        app.add_systems(FixedUpdate, reconcile_columns);
        let dim = app
            .world_mut()
            .spawn((ColumnIndex::default(), DimensionTypeConfig::new(0, 256)))
            .id();
        (app, dim)
    }

    fn spawn_section(app: &mut App, dim: Entity, pos: SectionPos, stage: SectionStage) -> Entity {
        let section = app.world_mut().spawn((pos, InDimension(dim), stage)).id();
        app.world_mut()
            .write_message(SectionStageChanged::spawned(section, pos, dim, stage));
        section
    }

    fn move_to(app: &mut App, section: Entity, to: SectionStage) {
        let world = app.world_mut();
        let pos = *world.get::<SectionPos>(section).expect("section pos");
        let dim = world.get::<InDimension>(section).expect("dimension").0;
        let from = world.get::<SectionStage>(section).copied();
        world.entity_mut(section).insert(to);
        world.write_message(SectionStageChanged {
            section,
            pos,
            dim,
            from,
            to,
        });
    }

    fn counted(app: &App, dim: Entity) -> Option<u32> {
        app.world()
            .get::<ColumnIndex>(dim)
            .expect("column index")
            .0
            .get(&ColumnPos::new(0, 0))
            .map(|slot| slot.section_count)
    }

    #[test]
    fn a_section_cancelled_before_it_loaded_does_not_decrement_its_column() {
        let (mut app, dim) = app_with_dimension();
        spawn_section(
            &mut app,
            dim,
            SectionPos::new(0, 0, 0),
            SectionStage::Loaded,
        );
        app.world_mut().run_schedule(FixedUpdate);

        let cancelled = spawn_section(
            &mut app,
            dim,
            SectionPos::new(0, 1, 0),
            SectionStage::Loading,
        );
        move_to(&mut app, cancelled, SectionStage::Unloading);
        app.world_mut().run_schedule(FixedUpdate);

        assert_eq!(counted(&app, dim), Some(1));
    }

    #[test]
    fn a_section_unloaded_twice_only_decrements_once() {
        let (mut app, dim) = app_with_dimension();
        let kept = spawn_section(
            &mut app,
            dim,
            SectionPos::new(0, 0, 0),
            SectionStage::Loaded,
        );
        let leaving = spawn_section(
            &mut app,
            dim,
            SectionPos::new(0, 1, 0),
            SectionStage::Loaded,
        );
        app.world_mut().run_schedule(FixedUpdate);
        assert_eq!(counted(&app, dim), Some(2));

        move_to(&mut app, leaving, SectionStage::Unloading);
        app.world_mut().run_schedule(FixedUpdate);
        assert_eq!(counted(&app, dim), Some(1));

        // A section waiting to despawn can be handed tickets and lose them again,
        // so the same section reaches `Unloading` a second time.
        move_to(&mut app, leaving, SectionStage::Loading);
        app.world_mut().run_schedule(FixedUpdate);
        move_to(&mut app, leaving, SectionStage::Unloading);
        app.world_mut().run_schedule(FixedUpdate);
        assert_eq!(counted(&app, dim), Some(1));

        let column = app.world().get::<ColumnIndex>(dim).expect("column index").0
            [&ColumnPos::new(0, 0)]
            .entity;
        assert_eq!(
            app.world()
                .get::<ColumnSections>(column)
                .expect("column sections")
                .lookup(0),
            SectionLookup::Loaded(kept)
        );
    }

    #[test]
    fn the_tick_and_the_drain_count_a_section_once() {
        #[derive(bevy_ecs::schedule::ScheduleLabel, Debug, Clone, PartialEq, Eq, Hash)]
        struct Drain;

        let (mut app, dim) = app_with_dimension();
        app.add_systems(Drain, reconcile_columns);
        let section = spawn_section(
            &mut app,
            dim,
            SectionPos::new(0, 0, 0),
            SectionStage::Loaded,
        );
        app.world_mut().run_schedule(FixedUpdate);
        app.world_mut().run_schedule(Drain);
        assert_eq!(counted(&app, dim), Some(1));

        move_to(&mut app, section, SectionStage::Unloading);
        app.world_mut().run_schedule(Drain);
        app.world_mut().run_schedule(FixedUpdate);
        assert_eq!(counted(&app, dim), None);
    }

    #[test]
    fn a_section_that_left_before_it_was_counted_leaves_no_column_behind() {
        let (mut app, dim) = app_with_dimension();
        let section = spawn_section(
            &mut app,
            dim,
            SectionPos::new(0, 0, 0),
            SectionStage::Loaded,
        );
        move_to(&mut app, section, SectionStage::Unloading);
        app.world_mut().run_schedule(FixedUpdate);

        assert_eq!(counted(&app, dim), None);
    }

    #[test]
    fn section_lookup_loaded() {
        let mut si = ColumnSections::new(-4, 24);
        let e = fake_entity(7);
        si.set_loaded(2, e);
        assert_eq!(si.lookup(2), SectionLookup::Loaded(e));
    }

    #[test]
    fn section_lookup_unloaded() {
        let si = ColumnSections::new(-4, 24);
        assert_eq!(si.lookup(0), SectionLookup::Unloaded);
    }

    #[test]
    fn section_lookup_just_below_range() {
        let si = ColumnSections::new(-4, 24);
        assert_eq!(si.lookup(-5), SectionLookup::OutOfRange);
    }

    #[test]
    fn section_lookup_just_above_range() {
        let si = ColumnSections::new(-4, 24);
        // min_section_y=-4, len=24 -> real range is -4..=19.
        assert_eq!(si.lookup(20), SectionLookup::OutOfRange);
    }

    #[test]
    fn section_lookup_out_of_range_low() {
        let si = ColumnSections::new(-4, 24);
        assert_eq!(si.lookup(-6), SectionLookup::OutOfRange);
    }

    #[test]
    fn section_lookup_out_of_range_high() {
        let si = ColumnSections::new(-4, 24);
        assert_eq!(si.lookup(21), SectionLookup::OutOfRange);
    }

    #[test]
    fn iter_length_equals_real_count() {
        let si = ColumnSections::new(-4, 24);
        assert_eq!(si.iter().count(), 24);
    }

    #[test]
    fn iter_passes_loaded_and_unloaded() {
        let mut si = ColumnSections::new(0, 3);
        let e = fake_entity(11);
        si.set_loaded(1, e);
        let collected: Vec<_> = si.iter().collect();
        assert_eq!(
            collected,
            vec![
                SectionLookup::Unloaded,
                SectionLookup::Loaded(e),
                SectionLookup::Unloaded,
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
        let cp = SectionPos::new(3, 7, -5);
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
