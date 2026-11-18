#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ModelId(pub(crate) u32);

impl ModelId {
    pub(crate) fn index(self) -> Option<usize> {
        usize::try_from(self.0).ok()
    }
}
