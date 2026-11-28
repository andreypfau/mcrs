use bevy_ecs::change_detection::DetectChanges;
use bevy_ecs::entity::Entity;
use bevy_ecs::message::MessageReader;
use bevy_ecs::prelude::{Commands, Component, Has, Populated, Query, Ref, Res, ResMut, With};
use bevy_ecs::resource::Resource;
use mcrs_minecraft_engine_world::block_update::BlockPlaced;
use mcrs_minecraft_engine_world::owned_columns::{self, OwnedColumns};
use mcrs_minecraft_engine_world::world::lifecycle::column_loading::ColumnLoading;
use mcrs_minecraft_engine_world::world::lifecycle::stage::SectionStage;
use mcrs_minecraft_engine_world::world::light_gate::{ColumnLightSettled, LightGated};
use mcrs_minecraft_engine_world::world::storage::block_entity::{BlockEntity, InSection};
use mcrs_minecraft_engine_world::world::storage::column::{
    ColumnIndex, ColumnSections, SectionLookup,
};

use crate::block_simulation::BlockSimulation;
use crate::column_asks::{ColumnHold, Hold};
use crate::column_snapshot::ColumnSnapshots;
use crate::column_store::ColumnStoreLink;
use crate::coordinator::clock::GameTime;
use crate::stored_columns_landing::TakenFromStore;
use mcrs_minecraft_simulation::GameTick;

#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaveRequest {
    Dirty,
    All,
}

#[derive(Component, Clone, Copy, Debug, Default)]
#[component(storage = "SparseSet")]
pub struct ColumnLightFinal;

#[derive(Component, Clone, Copy, Debug, Default)]
#[component(storage = "SparseSet")]
pub struct SectionChanged;

pub(crate) fn mark_final_light(
    mut settled: MessageReader<ColumnLightSettled>,
    indices: Query<&ColumnIndex>,
    mut commands: Commands,
) {
    for &ColumnLightSettled(column) in settled.read() {
        for index in &indices {
            if let Some(slot) = index.get(&column) {
                commands.entity(slot.entity).try_insert(ColumnLightFinal);
            }
        }
    }
}

pub(crate) fn mark_changed_sections(
    mut placed: MessageReader<BlockPlaced>,
    changed: Query<(), bevy_ecs::prelude::Without<SectionChanged>>,
    mut commands: Commands,
) {
    for placed in placed.read() {
        if changed.contains(placed.chunk) {
            commands.entity(placed.chunk).try_insert(SectionChanged);
        }
    }
}

pub(crate) fn mark_changed_block_entities(
    changed: Populated<(&InSection, Ref<BlockEntity>), bevy_ecs::prelude::Changed<BlockEntity>>,
    unmarked: Query<(), bevy_ecs::prelude::Without<SectionChanged>>,
    mut commands: Commands,
) {
    for (InSection(section), entity) in &changed {
        if entity.is_added() || !unmarked.contains(*section) {
            continue;
        }
        commands.entity(*section).try_insert(SectionChanged);
    }
}

type SectionMarks = (
    &'static SectionStage,
    Has<TakenFromStore>,
    Has<SectionChanged>,
);

type ColumnMarks = (&'static ColumnSections, Has<ColumnLightFinal>);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Origin {
    Unchanged,
    Kept,
    Generated,
}

pub(crate) fn columns_cooling(dims: Query<&ColumnLoading>) -> bool {
    dims.iter().any(|loading| !loading.cooling().is_empty())
}

#[cfg_attr(
    mcrs_lint,
    expect(
        mcrs::alloc_in_tick,
        reason = "each snapshot is built only for a column that cools in this tick, and it moves into the column store"
    )
)]
#[cfg_attr(mcrs_lint, mcrs::alloc_sites(6))]
pub(crate) fn write_cold_columns(
    dims: Query<(&ColumnLoading, &ColumnIndex)>,
    columns: Query<ColumnMarks>,
    marks: Query<SectionMarks>,
    snapshots: ColumnSnapshots,
    light_gated: Option<Res<LightGated>>,
    owned: Option<Res<OwnedColumns>>,
    mut store: ResMut<ColumnStoreLink>,
    mut simulation: Option<ResMut<BlockSimulation>>,
    time: Res<GameTime>,
) {
    let now = GameTick(time.0);
    for (loading, index) in &dims {
        for &column in loading.cooling() {
            if loading.is_column_loaded(column) || !owned_columns::owns(owned.as_deref(), column) {
                continue;
            }
            store.report(ColumnHold {
                column,
                hold: Hold::Cold,
            });
            let Some(slot) = index.get(&column) else {
                continue;
            };
            let Ok((column_sections, light_final)) = columns.get(slot.entity) else {
                continue;
            };
            let min_y = column_sections.min_section_y;
            let rows = (0..column_sections.sections.len())
                .map_while(|index| i32::try_from(index).ok()?.checked_add(min_y));
            let taken = simulation
                .as_mut()
                .map(|simulation| simulation.take_column(column, rows, now));
            let Some(origin) = origin(column_sections, &marks) else {
                continue;
            };
            let unlit = light_gated.is_some() && !light_final;
            let ticks_changed = taken.as_ref().is_some_and(|taken| taken.changed);
            match origin {
                Origin::Unchanged if !ticks_changed => continue,
                Origin::Generated if unlit && !ticks_changed => continue,
                Origin::Unchanged | Origin::Generated | Origin::Kept => {}
            }
            if let Some(mut stored) = snapshots.take_unlit(slot.entity) {
                stored.last_update = now.0;
                if let Some(taken) = taken {
                    stored.ticks = taken.ticks;
                }
                store.put(column, stored);
            }
        }
    }
}

#[cfg_attr(
    mcrs_lint,
    expect(
        mcrs::alloc_in_tick,
        reason = "runs only on a save request, and each snapshot moves into the column store"
    )
)]
#[cfg_attr(mcrs_lint, mcrs::alloc_sites(8))]
pub(crate) fn save_dirty_loaded_columns(
    dims: Query<(&ColumnLoading, &ColumnIndex)>,
    columns: Query<ColumnMarks>,
    marks: Query<SectionMarks>,
    snapshots: ColumnSnapshots,
    owned: Option<Res<OwnedColumns>>,
    mut store: ResMut<ColumnStoreLink>,
    simulation: Option<Res<BlockSimulation>>,
    time: Res<GameTime>,
    mut commands: Commands,
    request: Option<Res<SaveRequest>>,
) {
    if request.as_deref() != Some(&SaveRequest::Dirty) {
        return;
    }
    let now = GameTick(time.0);
    for (loading, index) in &dims {
        for column in loading.loaded_columns() {
            if !owned_columns::owns(owned.as_deref(), column) {
                continue;
            }
            let Some(slot) = index.get(&column) else {
                continue;
            };
            let Ok((column_sections, _)) = columns.get(slot.entity) else {
                continue;
            };
            let dirty = dirty_sections(column_sections, &marks);
            let Some(rows) = snapshots.rows(slot.entity) else {
                continue;
            };
            let copied = simulation
                .as_deref()
                .map(|simulation| simulation.copy_column(column, rows, now));
            let ticks_changed = copied.as_ref().is_some_and(|copied| copied.changed);
            if dirty.is_empty() && !ticks_changed {
                continue;
            }
            let Some(mut stored) = snapshots.take_unlit(slot.entity) else {
                continue;
            };
            stored.last_update = now.0;
            if let Some(copied) = copied {
                stored.ticks = copied.ticks;
            }
            store.put(column, stored);
            for section in dirty {
                commands.entity(section).remove::<SectionChanged>();
            }
        }
    }
    commands.remove_resource::<SaveRequest>();
}

fn dirty_sections(column: &ColumnSections, marks: &Query<SectionMarks>) -> Vec<Entity> {
    column
        .iter()
        .filter_map(|lookup| match lookup {
            SectionLookup::Loaded(section)
                if marks.get(section).is_ok_and(|(_, _, changed)| changed) =>
            {
                Some(section)
            }
            _ => None,
        })
        .collect()
}

#[cfg_attr(
    mcrs_lint,
    expect(
        mcrs::alloc_in_tick,
        reason = "runs only on a save request, and each snapshot moves into the column store"
    )
)]
#[cfg_attr(mcrs_lint, mcrs::alloc_sites(6))]
pub(crate) fn save_all_loaded_columns(
    dims: Query<(&ColumnLoading, &ColumnIndex)>,
    snapshots: ColumnSnapshots,
    owned: Option<Res<OwnedColumns>>,
    mut store: ResMut<ColumnStoreLink>,
    mut simulation: Option<ResMut<BlockSimulation>>,
    time: Res<GameTime>,
    mut commands: Commands,
    request: Option<Res<SaveRequest>>,
    changed_sections: Query<Entity, With<SectionChanged>>,
) {
    if request.as_deref() != Some(&SaveRequest::All) {
        return;
    }
    let now = GameTick(time.0);
    for (loading, index) in &dims {
        for column in loading.loaded_columns() {
            if !owned_columns::owns(owned.as_deref(), column) {
                continue;
            }
            let Some(slot) = index.get(&column) else {
                continue;
            };
            let Some(rows) = snapshots.rows(slot.entity) else {
                continue;
            };
            let taken = simulation
                .as_mut()
                .map(|simulation| simulation.take_column(column, rows, now));
            if let Some(mut stored) = snapshots.take_unlit(slot.entity) {
                stored.last_update = now.0;
                if let Some(taken) = taken {
                    stored.ticks = taken.ticks;
                }
                store.put(column, stored);
            }
        }
    }
    for section in &changed_sections {
        commands.entity(section).remove::<SectionChanged>();
    }
    commands.remove_resource::<SaveRequest>();
}

fn origin(column: &ColumnSections, marks: &Query<SectionMarks>) -> Option<Origin> {
    let mut any_taken = false;
    let mut all_taken = true;
    let mut any_changed = false;
    for lookup in column.iter() {
        let SectionLookup::Loaded(section) = lookup else {
            return None;
        };
        let (stage, taken, changed) = marks.get(section).ok()?;
        if *stage != SectionStage::Loaded {
            return None;
        }
        any_taken |= taken;
        all_taken &= taken;
        any_changed |= changed;
    }
    Some(if all_taken && !any_changed {
        Origin::Unchanged
    } else if any_taken || any_changed {
        Origin::Kept
    } else {
        Origin::Generated
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_app::App;
    use bevy_ecs::change_detection::DetectChangesMut;
    use mcrs_minecraft_block_entity::{ContainerData, GeneratedBlockEntity};
    use mcrs_voxel::BlockPos;

    #[derive(Resource, Default)]
    struct Dirty(Vec<Entity>);

    fn collect_dirty(
        columns: Query<&ColumnSections>,
        marks: Query<SectionMarks>,
        mut out: ResMut<Dirty>,
    ) {
        out.0.clear();
        for column in &columns {
            out.0 = dirty_sections(column, &marks);
        }
    }

    #[test]
    fn only_a_changed_section_is_dirty() {
        let mut app = App::new();
        app.init_resource::<Dirty>();
        app.add_systems(bevy_app::Update, collect_dirty);
        let section = app.world_mut().spawn(SectionStage::Loaded).id();
        let mut column = ColumnSections::new(0, 1);
        column.set_loaded(0, section);
        app.world_mut().spawn(column);
        app.update();
        assert!(app.world().resource::<Dirty>().0.is_empty());

        app.world_mut().entity_mut(section).insert(SectionChanged);
        app.update();
        assert_eq!(app.world().resource::<Dirty>().0, [section]);
    }

    #[test]
    fn a_block_entity_that_changed_marks_its_section_but_a_landed_one_does_not() {
        let mut app = App::new();
        app.add_systems(bevy_app::Update, mark_changed_block_entities);
        let section = app.world_mut().spawn_empty().id();
        let entity = app
            .world_mut()
            .spawn((
                InSection(section),
                BlockEntity(GeneratedBlockEntity::Chest(ContainerData::looted(
                    BlockPos::new(0, 0, 0),
                    "minecraft:chests/simple_dungeon".to_string(),
                    0,
                ))),
            ))
            .id();
        app.update();
        assert!(!app.world().entity(section).contains::<SectionChanged>());

        app.world_mut()
            .get_mut::<BlockEntity>(entity)
            .unwrap()
            .map_unchanged(|entity| &mut entity.0)
            .set_if_neq(GeneratedBlockEntity::Chest(ContainerData::looted(
                BlockPos::new(0, 0, 0),
                "minecraft:chests/abandoned_mineshaft".to_string(),
                1,
            )));
        app.update();
        assert!(app.world().entity(section).contains::<SectionChanged>());
    }
}
