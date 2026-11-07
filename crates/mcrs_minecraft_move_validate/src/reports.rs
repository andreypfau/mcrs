use crate::Finding;

const LAYER_0: [Finding; 5] = [
    Finding::PitchOutOfRange,
    Finding::PositionOutOfRange,
    Finding::UnknownMoveFlags,
    Finding::UnknownInputFlag,
    Finding::UnknownAbilityFlags,
];

/// The findings of layer 0 for one packet, as a set.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Reports(u16);

impl Reports {
    pub(crate) fn add(&mut self, finding: Finding) {
        self.0 |= bit(finding);
    }

    #[must_use]
    pub fn is_empty(self) -> bool {
        self.0 == 0
    }

    #[must_use]
    pub fn contains(self, finding: Finding) -> bool {
        self.0 & bit(finding) != 0
    }

    pub fn iter(self) -> impl Iterator<Item = Finding> {
        LAYER_0
            .into_iter()
            .filter(move |&finding| self.contains(finding))
    }
}

fn bit(finding: Finding) -> u16 {
    1 << finding as u16
}
