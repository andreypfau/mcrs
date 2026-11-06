/// The version of a chunk. Its owner gives each version it writes a higher number than the
/// last, so a higher version is always the later one.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ChunkVersion(pub u64);

impl ChunkVersion {
    /// The version after this one. It saturates, because no owner writes `u64::MAX` versions.
    #[must_use]
    pub fn next(self) -> Self {
        Self(self.0.saturating_add(1))
    }
}
