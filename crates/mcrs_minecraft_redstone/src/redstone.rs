use mcrs_minecraft_block::block_keys::Block;
use mcrs_minecraft_core::Direction;
use mcrs_minecraft_resource::Id;
use mcrs_minecraft_simulation::{
    BlockBehaviour, BlockFacts, BlockUpdateFlags, Cx, Kinds, SimulationVolume,
};
use mcrs_voxel::{BlockPos, BlockStateId};

use crate::component::Kind;
use crate::memory::{HasRedstoneMemory, HasRedstoneSnapshot};
use crate::rules::{RedstoneRules, WireEvaluator};
use crate::table::RedstoneTable;
use crate::{comparator, lamp, observer, piston, plate, repeater, source, torch, wire};
use crate::{daylight, mechanism, rail, tripwire, wire_network};
use mcrs_minecraft_simulation::Footprints;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Toggle {
    pub state: BlockStateId,
    pub flags: BlockUpdateFlags,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlockUse {
    Toggle(Toggle),
    Consumed,
    Unhandled,
}

pub trait HasRedstone {
    fn redstone(&self) -> &Redstone;

    fn redstone_mut(&mut self) -> &mut Redstone;
}

#[derive(Clone, Debug)]
pub struct Redstone {
    table: RedstoneTable,
    rules: RedstoneRules,
}

impl Redstone {
    #[must_use]
    pub fn new(table: RedstoneTable, rules: RedstoneRules) -> Self {
        Self { table, rules }
    }

    #[must_use]
    pub fn table(&self) -> &RedstoneTable {
        &self.table
    }

    #[must_use]
    pub fn rules(&self) -> RedstoneRules {
        self.rules
    }

    pub fn set_rules(&mut self, rules: RedstoneRules) {
        self.rules = rules;
    }

    #[must_use]
    pub fn reach(&self, state: BlockStateId) -> u8 {
        match self.table.kind(state) {
            Some(Kind::Rail { .. }) => rail::REACH,
            Some(Kind::Tripwire | Kind::TripwireHook) => tripwire::REACH,
            Some(Kind::Piston { .. }) => piston::REACH,
            Some(Kind::Wire) if self.rules.wire_evaluator == WireEvaluator::Experimental => {
                u8::try_from(wire_network::NETWORK_REACH).unwrap_or(Footprints::DEFAULT)
            }
            _ => Footprints::DEFAULT,
        }
    }

    #[must_use]
    pub fn handles(&self, state: BlockStateId) -> bool {
        self.table.kind(state).is_some()
    }

    #[must_use]
    pub fn used(&self, state: BlockStateId) -> BlockUse {
        let table = &self.table;
        let Some(kind) = table.kind(state) else {
            return BlockUse::Unhandled;
        };
        let toggled =
            match kind {
                Kind::Lever | Kind::Button { .. } => {
                    source::toggled_by_use(table, state).map(|state| (state, BlockUpdateFlags::ALL))
                }
                Kind::Repeater => repeater::toggled_by_use(table, state)
                    .map(|state| (state, BlockUpdateFlags::ALL)),
                Kind::Comparator => comparator::toggled_by_use(table, state)
                    .map(|state| (state, BlockUpdateFlags::CLIENTS)),
                Kind::Wire => {
                    wire::toggled_by_use(table, state).map(|state| (state, BlockUpdateFlags::ALL))
                }
                Kind::Door { wooden: true } | Kind::Trapdoor { wooden: true } | Kind::FenceGate => {
                    mechanism::toggled_by_use(table, state)
                        .map(|state| (state, BlockUpdateFlags::CLIENTS))
                }
                Kind::NoteBlock => mechanism::toggled_by_use(table, state)
                    .map(|state| (state, BlockUpdateFlags::ALL)),
                Kind::DaylightDetector => mechanism::toggled_by_use(table, state)
                    .map(|state| (state, BlockUpdateFlags::CLIENTS)),
                _ => return BlockUse::Unhandled,
            };
        match toggled {
            Some((state, flags)) => BlockUse::Toggle(Toggle { state, flags }),
            None if matches!(kind, Kind::Button { .. }) => BlockUse::Consumed,
            None => BlockUse::Unhandled,
        }
    }

    pub fn block_tick<K, V, F, B>(
        &self,
        cx: &mut Cx<'_, K, V, F, B>,
        at: BlockPos,
        state: BlockStateId,
    ) where
        K: Kinds<Block = Id<Block>>,
        V: SimulationVolume,
        F: BlockFacts<K>,
        B: BlockBehaviour<K>,
        B::Memory: HasRedstoneMemory,
        B::Shared: HasRedstoneSnapshot,
    {
        let table = &self.table;
        match table.kind(state) {
            Some(Kind::Button { .. }) => source::block_tick(cx, table, at, state),
            Some(Kind::Torch { .. }) => torch::block_tick(cx, table, at, state),
            Some(Kind::Lamp) => lamp::block_tick(cx, table, at, state),
            Some(Kind::Repeater) => repeater::block_tick(cx, table, at, state),
            Some(Kind::Comparator) => comparator::refresh(cx, table, at, state),
            Some(Kind::Observer) => observer::block_tick(cx, table, at, state),
            Some(Kind::Plate(_)) => plate::block_tick(cx, table, at, state),
            Some(Kind::MovingPiston) => piston::tick_complete(cx, table, at),
            Some(Kind::Tripwire) => tripwire::string_tick(cx, table, at, state),
            Some(Kind::TripwireHook) => tripwire::hook_tick(cx, table, at, state),
            _ => {}
        }
    }

    pub fn placed<K, V, F, B>(
        &self,
        cx: &mut Cx<'_, K, V, F, B>,
        at: BlockPos,
        new: BlockStateId,
        old: BlockStateId,
    ) where
        K: Kinds<Block = Id<Block>>,
        V: SimulationVolume,
        F: BlockFacts<K>,
        B: BlockBehaviour<K>,
        B::Memory: HasRedstoneMemory,
        B::Shared: HasRedstoneSnapshot,
    {
        let table = &self.table;
        match table.kind(new) {
            Some(Kind::Wire) => wire::placed(cx, table, &self.rules, at, new, old),
            Some(Kind::Lever | Kind::Button { .. }) => source::placed(cx, table, at, new, old),
            Some(Kind::Torch { .. }) => torch::placed(cx, table, at, new, old),
            Some(Kind::Lamp) => lamp::placed(cx, table, at, new, old),
            Some(Kind::Repeater) => repeater::placed(cx, table, at, new, old),
            Some(Kind::Comparator) => comparator::placed(cx, table, at, new, old),
            Some(Kind::Observer) => observer::placed(cx, table, at, new, old),
            Some(Kind::Plate(_)) if table.block_of(old) != table.block_of(new) => {
                plate::placed(cx, at);
            }
            Some(Kind::Piston { .. }) if table.block_of(old) != table.block_of(new) => {
                piston::check_extend(cx, table, &self.rules, at, new);
            }
            Some(Kind::Door { .. }) => mechanism::door_placed(cx, table, at, new, old),
            Some(
                Kind::Trapdoor { .. }
                | Kind::FenceGate
                | Kind::Hopper
                | Kind::Trigger(_)
                | Kind::CopperBulb,
            ) => mechanism::placed_powered(cx, table, &self.rules, at, new, old),
            Some(Kind::NoteBlock) => mechanism::note_placed(cx, table, at, new, old),
            Some(Kind::Rail { .. }) => rail::neighbour_changed(cx, table, at, new),
            Some(Kind::Tripwire) => tripwire::string_placed(cx, table, at, new, old),
            Some(Kind::TripwireHook) => tripwire::hook_placed(cx, table, at, new, old),
            Some(Kind::DaylightDetector) => daylight::placed(cx, table, at, new, old),
            _ => {}
        }
    }

    pub fn removed<K, V, F, B>(
        &self,
        cx: &mut Cx<'_, K, V, F, B>,
        at: BlockPos,
        old: BlockStateId,
        moved_by_piston: bool,
    ) where
        K: Kinds<Block = Id<Block>>,
        V: SimulationVolume,
        F: BlockFacts<K>,
        B: BlockBehaviour<K>,
        B::Memory: HasRedstoneMemory,
        B::Shared: HasRedstoneSnapshot,
    {
        let table = &self.table;
        match table.kind(old) {
            Some(Kind::Wire) => wire::removed(cx, table, at, moved_by_piston),
            Some(Kind::Lever | Kind::Button { .. }) => {
                source::removed(cx, table, at, old, moved_by_piston);
            }
            Some(Kind::Torch { .. }) => torch::removed(cx, table, at, old, moved_by_piston),
            Some(Kind::Repeater) => repeater::removed(cx, table, at, old, moved_by_piston),
            Some(Kind::Comparator) => comparator::removed(cx, table, at, old, moved_by_piston),
            Some(Kind::Observer) => observer::removed(cx, table, at, old, moved_by_piston),
            Some(Kind::Plate(_)) => plate::removed(cx, table, at, old, moved_by_piston),
            Some(Kind::MovingPiston) => piston::moving_removed(cx, at),
            Some(Kind::Tripwire) => tripwire::string_removed(cx, table, at, moved_by_piston),
            Some(Kind::TripwireHook) => {
                tripwire::hook_removed(cx, table, at, old, moved_by_piston);
            }
            Some(Kind::DaylightDetector) => daylight::removed(cx, at),
            _ => {}
        }
    }

    pub fn neighbour_changed<K, V, F, B>(
        &self,
        cx: &mut Cx<'_, K, V, F, B>,
        at: BlockPos,
        state: BlockStateId,
    ) where
        K: Kinds<Block = Id<Block>>,
        V: SimulationVolume,
        F: BlockFacts<K>,
        B: BlockBehaviour<K>,
        B::Memory: HasRedstoneMemory,
        B::Shared: HasRedstoneSnapshot,
    {
        let table = &self.table;
        match table.kind(state) {
            Some(Kind::Wire) => wire::neighbour_changed(cx, table, &self.rules, at, state),
            Some(Kind::Button { .. }) => source::neighbour_changed(cx, table, at, state),
            Some(Kind::Torch { .. }) => torch::neighbour_changed(cx, table, at, state),
            Some(Kind::Lamp) => lamp::neighbour_changed(cx, table, at, state),
            Some(Kind::Repeater) => repeater::neighbour_changed(cx, table, at, state),
            Some(Kind::Comparator) => comparator::neighbour_changed(cx, table, at, state),
            Some(Kind::Plate(_)) => plate::neighbour_changed(cx, table, at, state),
            Some(Kind::Piston { .. }) => piston::check_extend(cx, table, &self.rules, at, state),
            Some(Kind::PistonHead) => piston::head_neighbour_changed(cx, table, at, state),
            Some(Kind::MovingPiston) => piston::moving_neighbour_changed(cx, table, at),
            Some(Kind::Door { .. }) => mechanism::door_neighbour_changed(cx, table, at, state),
            Some(Kind::Trapdoor { .. } | Kind::FenceGate) => {
                mechanism::gate_neighbour_changed(cx, table, at, state);
            }
            Some(Kind::Hopper) => mechanism::hopper_neighbour_changed(cx, table, at, state),
            Some(Kind::Trigger(trigger)) => {
                mechanism::trigger_neighbour_changed(cx, table, &self.rules, at, state, trigger);
            }
            Some(Kind::CopperBulb) => mechanism::bulb_neighbour_changed(cx, table, at, state),
            Some(Kind::NoteBlock) => mechanism::note_neighbour_changed(cx, table, at, state),
            Some(Kind::Rail { .. }) => rail::neighbour_changed(cx, table, at, state),
            Some(Kind::Tripwire) => tripwire::string_neighbour_changed(cx, table, at, state),
            Some(Kind::DaylightDetector) => daylight::refresh(cx, table, at, state),
            _ => {}
        }
    }

    pub fn update_shape<K, V, F, B>(
        &self,
        cx: &mut Cx<'_, K, V, F, B>,
        at: BlockPos,
        state: BlockStateId,
        from: Direction,
        neighbour_state: BlockStateId,
    ) -> BlockStateId
    where
        K: Kinds<Block = Id<Block>>,
        V: SimulationVolume,
        F: BlockFacts<K>,
        B: BlockBehaviour<K>,
        B::Memory: HasRedstoneMemory,
        B::Shared: HasRedstoneSnapshot,
    {
        let table = &self.table;
        match table.kind(state) {
            Some(Kind::Wire) => {
                wire::update_shape(cx, table, &self.rules, at, state, from, neighbour_state)
            }
            Some(Kind::Lever | Kind::Button { .. }) => {
                source::update_shape(table, state, from, neighbour_state)
            }
            Some(Kind::Torch { .. }) => torch::update_shape(table, state, from, neighbour_state),
            Some(Kind::Repeater) => {
                repeater::update_shape(cx, table, at, state, from, neighbour_state)
            }
            Some(Kind::Comparator) => comparator::update_shape(table, state, from, neighbour_state),
            Some(Kind::Observer) => observer::update_shape(cx, table, at, state, from),
            Some(Kind::Plate(_)) => plate::update_shape(table, state, from, neighbour_state),
            Some(Kind::PistonHead) => {
                piston::head_update_shape(table, state, from, neighbour_state)
            }
            Some(Kind::Tripwire) => {
                tripwire::string_update_shape(table, state, from, neighbour_state)
            }
            Some(Kind::TripwireHook) => {
                tripwire::hook_update_shape(table, state, from, neighbour_state)
            }
            _ => state,
        }
    }

    pub fn block_event<K, V, F, B>(
        &self,
        cx: &mut Cx<'_, K, V, F, B>,
        at: BlockPos,
        state: BlockStateId,
        a: i32,
        b: i32,
    ) -> bool
    where
        K: Kinds<Block = Id<Block>>,
        V: SimulationVolume,
        F: BlockFacts<K>,
        B: BlockBehaviour<K>,
        B::Memory: HasRedstoneMemory,
        B::Shared: HasRedstoneSnapshot,
    {
        match self.table.kind(state) {
            Some(Kind::Piston { .. }) => {
                piston::block_event(cx, &self.table, &self.rules, at, state, a, b)
            }
            Some(Kind::NoteBlock) => mechanism::note_event(&self.table, state, a),
            _ => false,
        }
    }
}
