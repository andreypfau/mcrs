use mcrs_minecraft_block::block_keys::Block;
use mcrs_minecraft_core::Direction;
use mcrs_minecraft_resource::Id;
use mcrs_minecraft_simulation::{
    BlockBehaviour, BlockFacts, BlockUpdateFlags, Cx, Kinds, SimulationVolume, neighbour,
};
use mcrs_voxel::{BlockPos, BlockStateId};
use rustc_hash::FxHashMap;

use crate::component::Kind;
use crate::memory::{HasRedstoneMemory, HasRedstoneSnapshot};
use crate::power::{MAX_SIGNAL, WireSignals, best_neighbour_signal, neighbour_state, state_at};
use crate::table::RedstoneTable;
use crate::wire::LIMIT;

pub(crate) const NETWORK_REACH: i32 = 17;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct WireNode {
    pos: BlockPos,
    state: BlockStateId,
    external: u8,
    power: u8,
    frozen: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Feed {
    from: usize,
    to: usize,
}

#[derive(Clone, Debug, Default)]
struct WireScratch {
    nodes: Vec<WireNode>,
    index: FxHashMap<BlockPos, usize>,
    feeds: Vec<Feed>,
    changed: Vec<usize>,
}

impl WireScratch {
    fn admit(
        &mut self,
        table: &RedstoneTable,
        start: BlockPos,
        pos: BlockPos,
        state: BlockStateId,
    ) -> usize {
        if let Some(&index) = self.index.get(&pos) {
            return index;
        }
        let frozen = !within_reach(start, pos);
        let index = self.nodes.len();
        self.nodes.push(WireNode {
            pos,
            state,
            external: 0,
            power: if frozen { table.power(state) } else { 0 },
            frozen,
        });
        self.index.insert(pos, index);
        index
    }
}

fn within_reach(start: BlockPos, pos: BlockPos) -> bool {
    pos.x.abs_diff(start.x) <= NETWORK_REACH.unsigned_abs()
        && pos.y.abs_diff(start.y) <= NETWORK_REACH.unsigned_abs()
        && pos.z.abs_diff(start.z) <= NETWORK_REACH.unsigned_abs()
}

fn is_wire(table: &RedstoneTable, state: BlockStateId) -> bool {
    table.kind(state) == Some(Kind::Wire)
}

fn wire_at<K, V, F, B>(
    cx: &mut Cx<'_, K, V, F, B>,
    table: &RedstoneTable,
    pos: Option<BlockPos>,
) -> Option<(BlockPos, BlockStateId)>
where
    K: Kinds,
    V: SimulationVolume,
    F: BlockFacts<K>,
    B: BlockBehaviour<K>,
{
    let pos = pos?;
    let state = state_at(cx, pos)?;
    is_wire(table, state).then_some((pos, state))
}

fn inputs_of<K, V, F, B>(
    cx: &mut Cx<'_, K, V, F, B>,
    table: &RedstoneTable,
    pos: BlockPos,
    out: &mut Vec<(BlockPos, BlockStateId)>,
) where
    K: Kinds,
    V: SimulationVolume,
    F: BlockFacts<K>,
    B: BlockBehaviour<K>,
{
    let above_conductor =
        neighbour_state(cx, pos, Direction::Up).is_some_and(|(_, above)| table.is_conductor(above));
    for direction in Direction::HORIZONTAL {
        let Some((side, side_state)) = neighbour_state(cx, pos, direction) else {
            continue;
        };
        if let Some(found) = wire_at(cx, table, Some(side)) {
            out.push(found);
        }
        let vertical = if table.is_conductor(side_state) {
            if above_conductor {
                continue;
            }
            Direction::Up
        } else {
            Direction::Down
        };
        if let Some(found) = wire_at(cx, table, neighbour(side, vertical)) {
            out.push(found);
        }
    }
}

fn readers_of<K, V, F, B>(
    cx: &mut Cx<'_, K, V, F, B>,
    table: &RedstoneTable,
    pos: BlockPos,
    out: &mut Vec<(BlockPos, BlockStateId)>,
) where
    K: Kinds,
    V: SimulationVolume,
    F: BlockFacts<K>,
    B: BlockBehaviour<K>,
{
    let above = neighbour_state(cx, pos, Direction::Up);
    let below = neighbour_state(cx, pos, Direction::Down);
    let above_open = above.is_none_or(|(_, state)| !table.is_conductor(state));
    let below_solid = below.is_some_and(|(_, state)| table.is_conductor(state));
    for direction in Direction::HORIZONTAL {
        let Some(side) = neighbour(pos, direction) else {
            continue;
        };
        if let Some(found) = wire_at(cx, table, Some(side)) {
            out.push(found);
        }
        if above_open && let Some(found) = wire_at(cx, table, neighbour(side, Direction::Up)) {
            out.push(found);
        }
        if below_solid
            && state_at(cx, side).is_some_and(|state| !table.is_conductor(state))
            && let Some(found) = wire_at(cx, table, neighbour(side, Direction::Down))
        {
            out.push(found);
        }
    }
}

pub(crate) fn evaluate<K, V, F, B>(
    cx: &mut Cx<'_, K, V, F, B>,
    table: &RedstoneTable,
    start: BlockPos,
    state: BlockStateId,
) -> bool
where
    K: Kinds<Block = Id<Block>>,
    V: SimulationVolume,
    F: BlockFacts<K>,
    B: BlockBehaviour<K>,
    B::Memory: HasRedstoneMemory,
    B::Shared: HasRedstoneSnapshot,
{
    let mut scratch = WireScratch::default();
    scratch.admit(table, start, start, state);
    let mut found = Vec::new();
    let mut at = 0;
    while let Some(node) = scratch.nodes.get(at).copied() {
        if node.frozen {
            at = at.saturating_add(1);
            continue;
        }
        found.clear();
        inputs_of(cx, table, node.pos, &mut found);
        for &(pos, state) in &found {
            let from = scratch.admit(table, start, pos, state);
            scratch.feeds.push(Feed { from, to: at });
        }
        found.clear();
        readers_of(cx, table, node.pos, &mut found);
        for &(pos, state) in &found {
            let to = scratch.admit(table, start, pos, state);
            scratch.feeds.push(Feed { from: at, to });
        }
        let external = best_neighbour_signal(cx, table, node.pos, WireSignals::Ignore);
        if let Some(node) = scratch.nodes.get_mut(at) {
            node.external = external;
            node.power = external;
        }
        at = at.saturating_add(1);
    }
    relax(&mut scratch);
    for (index, node) in scratch.nodes.iter().enumerate() {
        if !node.frozen && table.power(node.state) != node.power {
            scratch.changed.push(index);
        }
    }
    let wire = Block::RedstoneWire.id();
    for &index in &scratch.changed {
        let Some(node) = scratch.nodes.get(index) else {
            continue;
        };
        if state_at(cx, node.pos) == Some(node.state)
            && let Some(next) = table.with_power(node.state, node.power)
        {
            cx.set(
                node.pos,
                next,
                BlockUpdateFlags::CLIENTS | BlockUpdateFlags::SKIP_SHAPE_UPDATE_ON_WIRE,
                LIMIT,
            );
        }
    }
    for &index in &scratch.changed {
        let Some(node) = scratch.nodes.get(index) else {
            continue;
        };
        cx.update_neighbours(node.pos, wire, None);
        for direction in Direction::all() {
            let Some(next) = neighbour(node.pos, direction) else {
                continue;
            };
            if !state_at(cx, next).is_some_and(|state| is_wire(table, state)) {
                cx.update_neighbours(next, wire, None);
            }
        }
    }
    true
}

fn relax(scratch: &mut WireScratch) {
    for _ in 0..=MAX_SIGNAL {
        let mut changed = false;
        for feed in &scratch.feeds {
            let Some(from) = scratch.nodes.get(feed.from).map(|node| node.power) else {
                continue;
            };
            let Some(to) = scratch.nodes.get_mut(feed.to) else {
                continue;
            };
            if to.frozen {
                continue;
            }
            let carried = from.saturating_sub(1);
            if carried > to.power {
                to.power = carried;
                changed = true;
            }
        }
        if !changed {
            return;
        }
    }
}
