//! A step between two air cells costs exactly one level, so their sky light
//! cannot differ by more than one. The server's answer satisfies that
//! everywhere, which makes it a check the client can run on its own — no sky
//! floors, no emission, no shapes, and no second light engine.
//!
//! It shares nothing with the server: a column and the eight around it are all
//! the rule reads, and the columns are exactly what the mesher already holds
//! alive, so the walk runs on a worker thread off a snapshot.

use std::collections::VecDeque;
use std::sync::Arc;

use bevy::platform::collections::{HashMap, HashSet};
use bevy::prelude::*;
use bevy::tasks::{AsyncComputeTaskPool, Task, futures::check_ready};
use mcrs_minecraft_network::columns::{BlockSource, ColumnStore, Neighbourhood, SECTION_SIZE};
use mcrs_minecraft_protocol::BlockStateId;
use mcrs_minecraft_world::block::definition::{BlockStateFlags, Blocks};
use mcrs_voxel_math::ColumnPos;

/// Columns handed to a worker per frame, and how many walks may be in flight.
/// One walk is 256 block columns of the world's height, so a handful of them is
/// about what a few section meshes cost.
const PER_FRAME: usize = 6;
const IN_FLIGHT: usize = 12;

/// Faults printed alongside the panic, so one crash shows the shape of the
/// break rather than a single cell of it.
const REPORTS: usize = 40;

pub struct LightGuardPlugin;

impl Plugin for LightGuardPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LightGuard>()
            .add_systems(Update, (enqueue, collect).chain());
    }
}

#[derive(Default, Resource)]
pub struct LightGuard {
    /// Whether each block state is air, by state id. Built once the block
    /// corpus exists; without it every cell would have to be treated as opaque
    /// and nothing would be checked.
    air: Option<Arc<[bool]>>,
    waiting: VecDeque<ColumnPos>,
    queued: HashSet<ColumnPos>,
    /// The column handle whose light already checked out. A light update
    /// replaces the handle, so a rewritten column is examined again.
    passed: HashMap<ColumnPos, usize>,
    running: Vec<Task<Report>>,
}

struct Report {
    pos: ColumnPos,
    handle: usize,
    faults: Vec<Fault>,
}

struct Fault {
    at: IVec3,
    sky: u8,
    state: u16,
    toward: IVec3,
    neighbour_sky: u8,
    neighbour_state: u16,
}

fn air_table(blocks: &Blocks) -> Arc<[bool]> {
    (0..blocks.state_count())
        .map(|index| {
            blocks
                .state(BlockStateId(index as u16))
                .flags
                .contains(BlockStateFlags::IS_AIR)
        })
        .collect()
}

fn surrounded(store: &ColumnStore, pos: ColumnPos) -> bool {
    (-1..=1).all(|dz| {
        (-1..=1).all(|dx| store.holds(ColumnPos::new(pos.x + dx, pos.z + dz)))
    })
}

fn enqueue(
    mut guard: ResMut<LightGuard>,
    store: Option<Res<ColumnStore>>,
    blocks: Option<Res<Blocks>>,
) {
    if !crate::config::light_guard() {
        return;
    }
    let Some(store) = store else {
        return;
    };
    if guard.air.is_none() {
        let Some(blocks) = blocks else {
            return;
        };
        guard.air = Some(air_table(&blocks));
    }
    let air = guard.air.clone().expect("the table was just built");

    for (pos, column) in store.resident() {
        let handle = Arc::as_ptr(column) as usize;
        if guard.passed.get(&pos) == Some(&handle) || guard.queued.contains(&pos) {
            continue;
        }
        guard.queued.insert(pos);
        guard.waiting.push_back(pos);
    }

    let pool = AsyncComputeTaskPool::get();
    let mut started = 0;
    while started < PER_FRAME && guard.running.len() < IN_FLIGHT {
        let Some(pos) = guard.waiting.pop_front() else {
            break;
        };
        if !store.holds(pos) {
            guard.queued.remove(&pos);
            continue;
        }
        if !surrounded(&store, pos) {
            // Put it back: a missing neighbour reads as open sky, which would
            // make every seam of the loading frontier look broken.
            guard.waiting.push_back(pos);
            break;
        }
        let handle = store
            .column(pos.x, pos.z)
            .map_or(0, |column| column as *const _ as usize);
        let neighbourhood = store.around(pos);
        let air = Arc::clone(&air);
        started += 1;
        guard
            .running
            .push(pool.spawn(async move { check(pos, handle, neighbourhood, air) }));
    }
}

fn collect(mut guard: ResMut<LightGuard>) {
    let mut finished = Vec::new();
    let mut running = std::mem::take(&mut guard.running);
    running.retain_mut(|task| match check_ready(task) {
        Some(report) => {
            finished.push(report);
            false
        }
        None => true,
    });
    guard.running = running;

    for report in finished {
        guard.queued.remove(&report.pos);
        if report.faults.is_empty() {
            guard.passed.insert(report.pos, report.handle);
            continue;
        }
        for fault in report.faults.iter().take(REPORTS) {
            error!(
                col_x = report.pos.x,
                col_z = report.pos.z,
                x = fault.at.x,
                y = fault.at.y,
                z = fault.at.z,
                sky = fault.sky,
                state = fault.state,
                nx = fault.toward.x,
                ny = fault.toward.y,
                nz = fault.toward.z,
                neighbour_sky = fault.neighbour_sky,
                neighbour_state = fault.neighbour_state,
                "sky light steps by more than one between two air cells"
            );
        }
        if !crate::config::light_guard_panics() {
            continue;
        }
        let first = &report.faults[0];
        panic!(
            "light guard: column {},{} carries sky light that cannot be a fixed point: \
             air at {} is {} while the air at {} is {} — one step apart, so they may differ \
             by at most one ({} faults in this column)",
            report.pos.x,
            report.pos.z,
            first.at,
            first.sky,
            first.toward,
            first.neighbour_sky,
            report.faults.len(),
        );
    }
}

fn check(pos: ColumnPos, handle: usize, world: Neighbourhood, air: Arc<[bool]>) -> Report {
    let mut faults = Vec::new();
    let Some(extent) = world.extent() else {
        return Report {
            pos,
            handle,
            faults,
        };
    };
    let is_air = |state: u16| air.get(state as usize).copied().unwrap_or(false);
    let bottom = extent.min_section_y * SECTION_SIZE as i32;
    let top = bottom + extent.sections as i32 * SECTION_SIZE as i32 - 1;

    for y in bottom..=top {
        for local_z in 0..SECTION_SIZE as i32 {
            for local_x in 0..SECTION_SIZE as i32 {
                let at = IVec3::new(
                    pos.x * SECTION_SIZE as i32 + local_x,
                    y,
                    pos.z * SECTION_SIZE as i32 + local_z,
                );
                if !is_air(world.block(at.x, at.y, at.z)) {
                    continue;
                }
                let sky = world.light(at.x, at.y, at.z) & 0x0f;
                for step in [
                    IVec3::X,
                    IVec3::NEG_X,
                    IVec3::Z,
                    IVec3::NEG_Z,
                    IVec3::Y,
                    IVec3::NEG_Y,
                ] {
                    let toward = at + step;
                    // Outside the world there are no blocks to read and the
                    // padding the wire carries is not part of the answer.
                    if toward.y < bottom || toward.y > top {
                        continue;
                    }
                    if !is_air(world.block(toward.x, toward.y, toward.z)) {
                        continue;
                    }
                    let neighbour_sky = world.light(toward.x, toward.y, toward.z) & 0x0f;
                    if sky.abs_diff(neighbour_sky) > 1 {
                        faults.push(Fault {
                            at,
                            sky,
                            state: world.block(at.x, at.y, at.z),
                            toward,
                            neighbour_sky,
                            neighbour_state: world.block(toward.x, toward.y, toward.z),
                        });
                        if faults.len() >= 64 {
                            return Report {
                                pos,
                                handle,
                                faults,
                            };
                        }
                    }
                }
            }
        }
    }

    Report {
        pos,
        handle,
        faults,
    }
}
