use mcrs_minecraft_volume::{BlockVolumeMut, Volume};
use mcrs_voxel::SectionPos;

bitflags::bitflags! {
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct SectionProperties: u8 {
        const ACCEPTS_TICKS = 1;
        const RUNS_SCHEDULED_TICKS = 2;
        const DELIVERS_BLOCK_EVENTS = 4;
        const BROADCASTS_CHANGES = 8;
    }
}

pub trait SimulatedSections: Volume {
    fn properties(&self, section: SectionPos) -> SectionProperties;

    /// The local indices of the positions of `section` whose state takes random ticks, kept with
    /// each write ([`TickingPositions`](crate::TickingPositions)). Empty for a section the host
    /// does not hold.
    fn random_ticking(&self, section: SectionPos) -> &[u16];
}

pub trait SimulationVolume: BlockVolumeMut + SimulatedSections {}

impl<V: BlockVolumeMut + SimulatedSections> SimulationVolume for V {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PartitionKind {
    Shard,
    Cell,
}
