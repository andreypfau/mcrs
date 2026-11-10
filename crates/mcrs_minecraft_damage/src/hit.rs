/// A hit on a living entity: the amount and the network id of the damage type. The source has no
/// entity and no position.
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub struct Hit {
    pub amount: f32,
    pub damage_type: u32,
}

impl Hit {
    #[must_use]
    pub const fn new(amount: f32, damage_type: u32) -> Self {
        Self {
            amount,
            damage_type,
        }
    }
}
