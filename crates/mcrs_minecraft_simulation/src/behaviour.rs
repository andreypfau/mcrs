use crate::SimulationVolume;
use crate::cx::Cx;
use crate::tick_queue::Kinds;
use mcrs_minecraft_core::{BlockPos, BlockStateId, Direction};

pub trait BlockFacts<K: Kinds> {
    fn block_of(&self, state: BlockStateId) -> K::Block;

    fn fluid_of(&self, state: BlockStateId) -> Option<K::Fluid>;

    fn void_air(&self) -> BlockStateId;

    fn is_air(&self, state: BlockStateId) -> bool;

    fn is_rail(&self, state: BlockStateId) -> bool;

    fn is_redstone_wire(&self, state: BlockStateId) -> bool;

    fn legacy_block_of_fluid(&self, state: BlockStateId) -> BlockStateId;
}

pub trait BlockBehaviour<K: Kinds>: Sync + Sized {
    fn block_tick<V: SimulationVolume, F: BlockFacts<K>>(
        &self,
        _cx: &mut Cx<'_, K, V, F, Self>,
        _at: BlockPos,
        _state: BlockStateId,
    ) {
    }

    fn fluid_tick<V: SimulationVolume, F: BlockFacts<K>>(
        &self,
        _cx: &mut Cx<'_, K, V, F, Self>,
        _at: BlockPos,
        _state: BlockStateId,
        _fluid: K::Fluid,
    ) {
    }

    fn block_event<V: SimulationVolume, F: BlockFacts<K>>(
        &self,
        _cx: &mut Cx<'_, K, V, F, Self>,
        _at: BlockPos,
        _state: BlockStateId,
        _a: i32,
        _b: i32,
    ) -> bool {
        false
    }

    fn removed<V: SimulationVolume, F: BlockFacts<K>>(
        &self,
        _cx: &mut Cx<'_, K, V, F, Self>,
        _at: BlockPos,
        _old: BlockStateId,
        _moved_by_piston: bool,
    ) {
    }

    fn placed<V: SimulationVolume, F: BlockFacts<K>>(
        &self,
        _cx: &mut Cx<'_, K, V, F, Self>,
        _at: BlockPos,
        _new: BlockStateId,
        _old: BlockStateId,
        _moved_by_piston: bool,
    ) {
    }

    fn neighbour_changed<V: SimulationVolume, F: BlockFacts<K>>(
        &self,
        _cx: &mut Cx<'_, K, V, F, Self>,
        _at: BlockPos,
        _state: BlockStateId,
        _source: K::Block,
        _moved_by_piston: bool,
    ) {
    }

    fn update_shape<V: SimulationVolume, F: BlockFacts<K>>(
        &self,
        _cx: &mut Cx<'_, K, V, F, Self>,
        _at: BlockPos,
        state: BlockStateId,
        _from: Direction,
        _neighbour: BlockPos,
        _neighbour_state: BlockStateId,
    ) -> BlockStateId {
        state
    }
}
