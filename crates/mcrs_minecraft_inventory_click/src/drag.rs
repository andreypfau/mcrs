use crate::DragKind;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Drag {
    kind: Option<DragKind>,
    entries: u128,
}

impl Drag {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            kind: None,
            entries: 0,
        }
    }

    #[must_use]
    pub fn kind(&self) -> Option<DragKind> {
        self.kind
    }

    pub fn entries(&self) -> impl Iterator<Item = u16> + '_ {
        (0..u128::BITS as u16).filter(|index| self.contains(*index))
    }

    pub(crate) fn len(&self) -> u32 {
        self.entries.count_ones()
    }

    pub(crate) fn open(&mut self, kind: DragKind) {
        *self = Self {
            kind: Some(kind),
            entries: 0,
        };
    }

    pub(crate) fn reset(&mut self) {
        *self = Self::new();
    }

    pub(crate) fn insert(&mut self, index: u16) {
        self.entries |= bit(index);
    }

    fn contains(&self, index: u16) -> bool {
        self.entries & bit(index) != 0
    }
}

fn bit(index: u16) -> u128 {
    1u128.checked_shl(u32::from(index)).unwrap_or(0)
}
