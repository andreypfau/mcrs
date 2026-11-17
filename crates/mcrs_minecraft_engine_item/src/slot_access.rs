use crate::Gate;
use crate::capacity::SLOT_LIMIT;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SlotAccess {
    pub limit: u8,
    pub take: Gate,
    pub place: Gate,
}

impl SlotAccess {
    pub(crate) const OPEN: Self = Self {
        limit: SLOT_LIMIT,
        take: Gate::Permits,
        place: Gate::Permits,
    };

    pub(crate) fn clamped(self) -> Self {
        Self {
            limit: self.limit.clamp(1, SLOT_LIMIT),
            ..self
        }
    }
}
