use mcrs_minecraft_entity_attribute::HomeSlot;
use mcrs_minecraft_entity_attribute::Modifier;
use mcrs_minecraft_entity_attribute::ModifierOperation;
use mcrs_minecraft_resource::ResourceLocation;
use mcrs_minecraft_resource::ResourceLocationBuf;

use crate::modifier_kind::ModifierKind;

/// A modifier of an instance: an identifier, an amount, an operation and a kind
/// (`attributes.md`, ATR-INS-003 and ATR-INS-005), with the home slot of its identifier.
#[derive(Clone, Debug, PartialEq)]
pub struct InstanceModifier {
    id: ResourceLocationBuf,
    amount: f64,
    operation: ModifierOperation,
    kind: ModifierKind,
    slot: HomeSlot,
}

impl InstanceModifier {
    #[must_use]
    pub fn new(
        id: ResourceLocationBuf,
        amount: f64,
        operation: ModifierOperation,
        kind: ModifierKind,
    ) -> Self {
        let slot = HomeSlot::from(id.as_location());
        Self {
            id,
            amount,
            operation,
            kind,
            slot,
        }
    }

    #[must_use]
    pub fn id(&self) -> ResourceLocation<'_> {
        self.id.as_location()
    }

    #[must_use]
    pub fn amount(&self) -> f64 {
        self.amount
    }

    #[must_use]
    pub fn operation(&self) -> ModifierOperation {
        self.operation
    }

    #[must_use]
    pub fn kind(&self) -> ModifierKind {
        self.kind
    }

    /// Equal under ATR-INS-004: the same identifier, amount bits, operation and kind.
    pub(crate) fn same_as(&self, other: &Self) -> bool {
        self.id == other.id
            && self.amount.to_bits() == other.amount.to_bits()
            && self.operation == other.operation
            && self.kind == other.kind
    }

    pub(crate) fn to_modifier(&self) -> Modifier {
        Modifier {
            operation: self.operation,
            amount: self.amount,
            slot: self.slot,
        }
    }
}
