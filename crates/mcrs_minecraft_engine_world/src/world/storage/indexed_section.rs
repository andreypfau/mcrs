use mcrs_minecraft_simulation::TickingPositions;
use mcrs_voxel::SectionPos;

#[derive(Debug)]
pub(crate) struct IndexedSection {
    pub(crate) pos: SectionPos,
    pub(crate) random_ticked: bool,
    pub(crate) ticking: TickingPositions,
}

impl IndexedSection {
    pub(crate) fn is_listed(&self) -> bool {
        self.random_ticked && !self.ticking.as_slice().is_empty()
    }
}
