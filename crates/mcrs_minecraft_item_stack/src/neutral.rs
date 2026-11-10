use crate::ItemComponent;
use crate::Rarity;
use crate::RepairCost;

/// A component type with a value that a stack entity does not store.
///
/// A stack that has the neutral value and a stack that has no stored value of the type are the
/// same for the merge check, the row hash and the value form.
pub trait Neutral: ItemComponent + 'static {
    const NEUTRAL: &'static Self;
}

impl Neutral for Rarity {
    const NEUTRAL: &'static Self = &Self::Common;
}

impl Neutral for RepairCost {
    const NEUTRAL: &'static Self = &Self(0);
}
