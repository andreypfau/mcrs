use mcrs_voxel::BlockStateId;

/// The footprint bound of the handlers of each block state (SIM-MODEL-009), on the horizontal
/// axes only: a block tile covers the whole height. One reach covers every handler kind of a
/// state, so it is the largest bound of Tables 1 to 7 for its block type.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Footprints {
    reach: Vec<u8>,
}

impl Footprints {
    /// The default bound of shape functions, side effects and scheduled ticks (Tables 4 to 6).
    pub const DEFAULT: u8 = 2;
    /// A handler with no bound: it runs only in the serial step.
    pub const UNBOUNDED: u8 = u8::MAX;

    pub fn new(
        states: impl IntoIterator<Item = BlockStateId>,
        mut reach_of: impl FnMut(BlockStateId) -> u8,
    ) -> Self {
        let mut footprints = Self::default();
        for state in states {
            let reach = reach_of(state);
            if reach == Self::DEFAULT {
                continue;
            }
            let index = state.index();
            if footprints.reach.len() <= index {
                footprints
                    .reach
                    .resize(index.saturating_add(1), Self::DEFAULT);
            }
            if let Some(slot) = footprints.reach.get_mut(index) {
                *slot = reach;
            }
        }
        footprints
    }

    /// The largest horizontal distance, in blocks, from the position of a handler call of
    /// `state` to a position that the call reads or writes.
    #[must_use]
    pub fn reach(&self, state: BlockStateId) -> u8 {
        self.reach
            .get(state.index())
            .copied()
            .unwrap_or(Self::DEFAULT)
    }
}
