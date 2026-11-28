use mcrs_voxel::BlockStateId;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Footprints {
    reach: Vec<u8>,
}

impl Footprints {
    pub const DEFAULT: u8 = 2;
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

    #[must_use]
    pub fn reach(&self, state: BlockStateId) -> u8 {
        self.reach
            .get(state.index())
            .copied()
            .unwrap_or(Self::DEFAULT)
    }
}
