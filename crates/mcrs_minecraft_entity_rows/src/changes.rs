use crate::Crossing;
use crate::Placed;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Changes {
    pub(crate) spawned: Vec<Placed>,
    pub(crate) crossed: Vec<Crossing>,
    pub(crate) removed: Vec<Placed>,
}

impl Changes {
    #[must_use]
    pub fn spawned(&self) -> &[Placed] {
        &self.spawned
    }

    #[must_use]
    pub fn crossed(&self) -> &[Crossing] {
        &self.crossed
    }

    #[must_use]
    pub fn removed(&self) -> &[Placed] {
        &self.removed
    }
}
