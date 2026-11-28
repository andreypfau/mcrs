use core::mem;

use bevy_ecs::change_detection::DetectChanges;
use bevy_ecs::change_detection::DetectChangesMut;
use bevy_ecs::change_detection::Tick;
use bevy_ecs::lifecycle::RemovedComponents;
use bevy_ecs::system::Local;
use bevy_ecs::system::Query;
use bevy_ecs::system::ResMut;
use bevy_ecs::world::Mut;
use bevy_ecs::world::Ref;
use bevy_tasks::ComputeTaskPool;
use bevy_tasks::TaskPool;
use mcrs_minecraft_damage::Vitals;
use mcrs_minecraft_entity_key::TileKey;
use mcrs_minecraft_physics::BodyGrid;
use mcrs_minecraft_physics::GridRow;
use mcrs_minecraft_physics::TileBodies;
use mcrs_minecraft_physics::grid_row;

use crate::MotionId;
use crate::MotionKind;
use crate::MotionState;
use crate::grid_slot::GridSlot;
use crate::living_flags::living_flags;
use crate::regrids::Regrids;
use crate::tile_edits::Arrival;
use crate::tile_edits::Departure;
use crate::tile_edits::PARTS;
use crate::tile_edits::SlotChange;
use crate::tile_edits::TileBatch;
use crate::tile_edits::TileEdits;

type Slotted<'a> = (
    Option<Ref<'a, MotionState>>,
    Option<Ref<'a, MotionKind>>,
    Option<Ref<'a, MotionId>>,
    Option<Ref<'a, Vitals>>,
    Mut<'a, GridSlot>,
);

#[derive(Default)]
pub(crate) struct GridOwner {
    outside_step: TileEdits,
    parts: Vec<PartWork>,
}

#[derive(Default)]
struct PartWork {
    incoming: Vec<TileBatch>,
    gone: Vec<Departure>,
    added: Vec<Arrival>,
    rows: Vec<GridRow>,
    tiles: Vec<TileWork>,
    filled: usize,
}

#[derive(Default)]
struct TileWork {
    tile: TileKey,
    bodies: TileBodies,
    reach: f32,
}

#[cfg_attr(
    mcrs_lint,
    expect(
        mcrs::ungated_system,
        reason = "each tick drains the removal readers and the departures, and regrids every body whose state changed outside the step"
    )
)]
pub(crate) fn own_body_grid(
    mut grid: ResMut<BodyGrid>,
    mut regrids: ResMut<Regrids>,
    mut slots: Query<(
        Option<Ref<MotionState>>,
        Option<Ref<MotionKind>>,
        Option<Ref<MotionId>>,
        Option<Ref<Vitals>>,
        &mut GridSlot,
    )>,
    mut lost_ids: RemovedComponents<MotionId>,
    mut lost_states: RemovedComponents<MotionState>,
    mut lost_kinds: RemovedComponents<MotionKind>,
    mut owner: Local<GridOwner>,
) {
    let GridOwner {
        outside_step,
        parts,
    } = &mut *owner;
    let Regrids {
        queues,
        stepped,
        departures,
    } = &mut *regrids;
    apply(parts, queues.iter_mut(), &mut grid);
    for row in departures.drain(..) {
        outside_step.leave(&row);
    }
    let lost = (lost_ids.read())
        .chain(lost_states.read())
        .chain(lost_kinds.read());
    for entity in lost {
        if let Ok(slotted) = slots.get_mut(entity) {
            regrid(outside_step, slotted);
        }
    }
    let stepped = *stepped;
    let edits = &*queues;
    slots.par_iter_mut().for_each(|slotted| {
        if touched(&slotted, stepped) {
            edits.scope(|edits| regrid(edits, slotted));
        }
    });
    apply(parts, queues.iter_mut().chain([outside_step]), &mut grid);
}

fn touched((state, kind, id, vitals, _): &Slotted<'_>, stepped: Tick) -> bool {
    state
        .as_ref()
        .is_some_and(|state| state.is_changed() && state.last_changed() != stepped)
        || vitals
            .as_ref()
            .is_some_and(|vitals| vitals.is_changed() && vitals.last_changed() != stepped)
        || kind.as_ref().is_some_and(DetectChanges::is_changed)
        || id.as_ref().is_some_and(DetectChanges::is_changed)
}

fn regrid(edits: &mut TileEdits, (state, kind, id, vitals, mut slot): Slotted<'_>) {
    let row = match (state, kind, id) {
        (Some(state), Some(kind), Some(id)) => {
            grid_row(&state.0, &kind.0, id.0, living_flags(vitals.as_deref()))
        }
        _ => None,
    };
    if slot.0 == row {
        return;
    }
    edits.push(SlotChange {
        old: slot.0,
        new: row,
    });
    slot.bypass_change_detection().0 = row;
}

fn apply<'a>(
    parts: &mut Vec<PartWork>,
    sources: impl IntoIterator<Item = &'a mut TileEdits>,
    grid: &mut BodyGrid,
) {
    if parts.len() < PARTS {
        parts.resize_with(PARTS, PartWork::default);
    }
    let mut edited = false;
    for (source_index, source) in sources.into_iter().enumerate() {
        for (part, batch) in parts.iter_mut().zip(source.parts_mut()) {
            if batch.is_empty() {
                continue;
            }
            edited = true;
            if part.incoming.len() <= source_index {
                part.incoming
                    .resize_with(source_index + 1, TileBatch::default);
            }
            if let Some(incoming) = part.incoming.get_mut(source_index) {
                mem::swap(incoming, batch);
            }
        }
    }
    if !edited {
        return;
    }
    let pool = ComputeTaskPool::get_or_init(TaskPool::default);
    let read: &BodyGrid = grid;
    pool.scope(|scope| {
        for part in parts.iter_mut() {
            scope.spawn(async move { part.build(read) });
        }
    });
    for part in parts.iter_mut() {
        let filled = part.tiles.get_mut(..part.filled).unwrap_or_default();
        for work in filled {
            grid.swap(work.tile, &mut work.bodies);
            grid.widen(work.reach);
        }
        part.filled = 0;
    }
}

impl PartWork {
    fn build(&mut self, grid: &BodyGrid) {
        let Self {
            incoming,
            gone,
            added,
            rows,
            tiles,
            filled,
        } = self;
        gone.clear();
        added.clear();
        for batch in incoming.iter_mut() {
            gone.append(&mut batch.gone);
            added.append(&mut batch.added);
        }
        gone.sort_unstable();
        gone.dedup();
        added.sort_unstable_by_key(|arrival| arrival.tile);
        *filled = 0;
        let (mut gone, mut added) = (&gone[..], &added[..]);
        while let Some(tile) = next_tile(gone, added) {
            let left = gone.partition_point(|departure| departure.tile <= tile);
            let (here_gone, rest_gone) = gone.split_at(left);
            let arrived = added.partition_point(|arrival| arrival.tile <= tile);
            let (here_added, rest_added) = added.split_at(arrived);
            (gone, added) = (rest_gone, rest_added);
            rows.clear();
            let kept = (grid.tile(tile).rows().iter()).filter(|row| {
                here_gone
                    .binary_search_by_key(&row.key, |departure| departure.key)
                    .is_err()
            });
            rows.extend(kept);
            rows.extend(here_added.iter().map(|arrival| arrival.row));
            if tiles.len() <= *filled {
                tiles.push(TileWork::default());
            }
            if let Some(work) = tiles.get_mut(*filled) {
                work.tile = tile;
                work.reach = work.bodies.fill(tile, rows);
                *filled += 1;
            }
        }
    }
}

fn next_tile(gone: &[Departure], added: &[Arrival]) -> Option<TileKey> {
    match (gone.first(), added.first()) {
        (Some(departure), Some(arrival)) => Some(departure.tile.min(arrival.tile)),
        (Some(departure), None) => Some(departure.tile),
        (None, Some(arrival)) => Some(arrival.tile),
        (None, None) => None,
    }
}
