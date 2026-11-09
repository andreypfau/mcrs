use crate::BirthKey;

/// What an entity keeps of its birth, in its save and across a change of dimension: its key and
/// its phase offset. A parity test sets the phase from the vanilla network id, so the phase is
/// saved and not derived again.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Birth {
    pub key: BirthKey,
    /// From 0 to 3.
    pub phase: u8,
}

impl From<BirthKey> for Birth {
    fn from(key: BirthKey) -> Self {
        Self {
            key,
            phase: key.phase_offset(),
        }
    }
}
