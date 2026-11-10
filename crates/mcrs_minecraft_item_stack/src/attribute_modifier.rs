use core::hash::Hash;
use core::hash::Hasher;

use mcrs_minecraft_resource::ResourceLocation;
use mcrs_minecraft_resource::ResourceLocationBuf;

use crate::AttributeId;
use crate::ModifierDisplay;
use crate::ModifierOperation;
use crate::SlotGroup;
use crate::float_key;
use crate::identifier::Identifier;

/// One change that a stack makes to an attribute of the entity that has it equipped.
#[derive(Clone, Debug)]
pub struct AttributeModifier {
    pub attribute: AttributeId,
    id: Identifier,
    pub amount: f64,
    pub operation: ModifierOperation,
    pub slot: SlotGroup,
    pub display: ModifierDisplay,
}

impl AttributeModifier {
    /// A modifier for each slot with the default display.
    #[must_use]
    pub const fn new(
        attribute: AttributeId,
        id: ResourceLocationBuf,
        amount: f64,
        operation: ModifierOperation,
    ) -> Self {
        Self::with_id(attribute, Identifier::Owned(id), amount, operation)
    }

    /// A modifier for each slot with the default display.
    #[must_use]
    pub const fn from_static(
        attribute: AttributeId,
        id: ResourceLocation<'static>,
        amount: f64,
        operation: ModifierOperation,
    ) -> Self {
        Self::with_id(attribute, Identifier::Borrowed(id), amount, operation)
    }

    /// The name of the modifier. It is not an entry of a registry.
    #[must_use]
    pub fn id(&self) -> ResourceLocation<'_> {
        self.id.as_location()
    }

    const fn with_id(
        attribute: AttributeId,
        id: Identifier,
        amount: f64,
        operation: ModifierOperation,
    ) -> Self {
        Self {
            attribute,
            id,
            amount,
            operation,
            slot: SlotGroup::Any,
            display: ModifierDisplay::Default,
        }
    }
}

impl PartialEq for AttributeModifier {
    fn eq(&self, other: &Self) -> bool {
        self.attribute == other.attribute
            && self.id == other.id
            && float_key::of_f64(self.amount) == float_key::of_f64(other.amount)
            && self.operation == other.operation
            && self.slot == other.slot
            && self.display == other.display
    }
}

impl Hash for AttributeModifier {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.attribute.hash(state);
        self.id.hash(state);
        float_key::of_f64(self.amount).hash(state);
        self.operation.hash(state);
        self.slot.hash(state);
        self.display.hash(state);
    }
}
