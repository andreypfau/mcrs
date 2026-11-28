use fixedbitset::FixedBitSet;
use mcrs_minecraft_block::definition::FluidState;
use mcrs_minecraft_block::fluid_keys::Fluid;
use mcrs_voxel::BlockPos;

use crate::location_view::LocationView;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct BakedFluidPredicate {
    pub(crate) fluids: Option<FixedBitSet>,
    pub(crate) states: Option<FluidStates>,
}

impl BakedFluidPredicate {
    #[must_use]
    pub(crate) fn matches<W: LocationView>(&self, world: &W, pos: BlockPos) -> bool {
        if !world.is_loaded(pos) {
            return false;
        }
        let Some(fluid) = world.fluid_state(pos) else {
            return false;
        };
        self.fluids
            .as_ref()
            .is_none_or(|fluids| fluids.contains(fluid.fluid.index()))
            && self.states.is_none_or(|states| states.contains(&fluid))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct FluidStates(u16);

impl FluidStates {
    pub(crate) fn from_test(test: impl Fn(u8, bool) -> bool) -> Self {
        let mut bits = 0;
        for level in FluidState::LEVELS {
            for source in [false, true] {
                if let Some(bit) = Self::bit(level, source)
                    && test(level, source)
                {
                    bits |= bit;
                }
            }
        }
        FluidStates(bits)
    }

    fn contains(self, state: &FluidState) -> bool {
        state.fluid != Fluid::Empty.id()
            && Self::bit(state.level, state.source).is_some_and(|bit| self.0 & bit != 0)
    }

    fn bit(level: u8, source: bool) -> Option<u16> {
        if !FluidState::LEVELS.contains(&level) {
            return None;
        }
        let index = (level - 1) * 2 + u8::from(source);
        Some(1 << index)
    }
}
