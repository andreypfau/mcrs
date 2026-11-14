use crate::AttributeModifier;
use crate::HomeSlot;
use crate::ModifierOperation;

/// What the value computation reads of one modifier.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Modifier {
    pub operation: ModifierOperation,
    pub amount: f64,
    pub slot: HomeSlot,
}

impl From<AttributeModifier<'_>> for Modifier {
    fn from(modifier: AttributeModifier<'_>) -> Self {
        Self {
            operation: modifier.operation,
            amount: modifier.amount,
            slot: HomeSlot::from(modifier.id),
        }
    }
}
