use bevy_ecs::message::MessageReader;
use bevy_ecs::prelude::{Commands, Component, Has, Query, Res, ResMut};
use mcrs_minecraft_engine_world::block_update::BlockPlaced;
use mcrs_minecraft_engine_world::owned_columns::{self, OwnedColumns};
use mcrs_minecraft_engine_world::world::lifecycle::column_loading::ColumnLoading;
use mcrs_minecraft_engine_world::world::lifecycle::stage::SectionStage;
use mcrs_minecraft_engine_world::world::light_gate::{ColumnLightSettled, LightGated};
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

/// The light of the column, and of the eight around it, settled while it was loaded, so the
/// light it holds is the light it keeps when it goes cold.
#[derive(Component, Clone, Copy, Debug, Default)]
#[component(storage = "SparseSet")]
pub struct ColumnLightFinal;

/// A block of the section changed since it was generated or taken from the data layer.
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

type SectionMarks = (
    &'static SectionStage,
    Has<TakenFromStore>,
    Has<SectionChanged>,
);

type ColumnMarks = (&'static ColumnSections, Has<ColumnLightFinal>);

/// Where the content of a column came from since it was loaded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Origin {
    /// Every section came back from the data layer, and none changed.
    Unchanged,
    /// Some section came back from the data layer, or some section changed.
    Kept,
    /// Every section was generated, and none changed.
    Generated,
}

/// Writes the last version of each column that stopped being loaded, while its sections still
/// hold their blocks.
///
/// A column that never landed whole keeps the version the data layer already holds, and so
/// does a column that the data layer handed back and that did not change. A generated column
/// whose light never settled is not written: its light at the edge of the loaded space was
/// lit without the neighbours, and the data layer generates the same column again. Either column
/// is written still when its scheduled ticks changed since it landed, so that they are kept.
#[expect(
    clippy::too_many_arguments,
    reason = "the write reads the marks, the content and the ticks of each column that cools"
)]
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
            // A column of the halo is a copy: its owner writes it.
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
            // The sections go now, so their ticks leave the queues whether or not they are kept.
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
            // The light of the dimension may lag the last change, so the data layer lights the
            // column again from its blocks before a view gets it.
            if let Some(mut stored) = snapshots.take_unlit(slot.entity) {
                if let Some(taken) = taken {
                    stored.ticks = taken.ticks;
                }
                store.put(column, stored);
            }
        }
    }
}

/// Where the content of a column came from, or `None` when a section has not landed.
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
