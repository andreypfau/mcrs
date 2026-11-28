use crate::ItemComponent;
use crate::Rarity;
use crate::RepairCost;

pub trait Neutral: ItemComponent + 'static {
    const NEUTRAL: &'static Self;
}

impl Neutral for Rarity {
    const NEUTRAL: &'static Self = &Self::Common;
}

impl Neutral for RepairCost {
    const NEUTRAL: &'static Self = &Self(0);
}
