extern crate alloc;
use alloc::vec::Vec;

use mcrs_minecraft_entity::attribute_keys::Attribute;
use mcrs_minecraft_resource::ResourceLocationBuf;

/// The operation of an attribute modifier.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum AttributeOperation {
    AddValue = 0,
    AddMultipliedBase = 1,
    AddMultipliedTotal = 2,
}

/// A modifier applied to an attribute instance.
#[derive(Clone, Debug, PartialEq)]
pub struct AttributeModifier {
    pub id: ResourceLocationBuf,
    pub amount: f64,
    pub operation: AttributeOperation,
}

impl AttributeModifier {
    #[must_use]
    pub fn new(id: ResourceLocationBuf, amount: f64, operation: AttributeOperation) -> Self {
        Self { id, amount, operation }
    }
}

/// An attribute instance with a base value and applied modifiers.
#[derive(Clone, Debug, PartialEq)]
pub struct AttributeInstance {
    pub attribute: Attribute,
    pub base_value: f64,
    pub modifiers: Vec<AttributeModifier>,
}

impl AttributeInstance {
    #[must_use]
    pub fn new(attribute: Attribute, base_value: f64) -> Self {
        Self {
            attribute,
            base_value,
            modifiers: Vec::new(),
        }
    }

    #[must_use]
    pub fn with_default(attribute: Attribute) -> Self {
        Self::new(attribute, attribute.definition().default)
    }

    pub fn add_modifier(&mut self, modifier: AttributeModifier) {
        self.remove_modifier(&modifier.id);
        self.modifiers.push(modifier);
    }

    pub fn remove_modifier(&mut self, id: &ResourceLocationBuf) {
        self.modifiers.retain(|m| &m.id != id);
    }

    /// Computes the final value of the attribute clamped to its definition limits.
    #[must_use]
    pub fn value(&self) -> f64 {
        let mut add_value = self.base_value;
        for modifier in &self.modifiers {
            if modifier.operation == AttributeOperation::AddValue {
                add_value += modifier.amount;
            }
        }

        let mut multiplied_base = add_value;
        for modifier in &self.modifiers {
            if modifier.operation == AttributeOperation::AddMultipliedBase {
                multiplied_base += add_value * modifier.amount;
            }
        }

        let mut total = multiplied_base;
        for modifier in &self.modifiers {
            if modifier.operation == AttributeOperation::AddMultipliedTotal {
                total *= 1.0 + modifier.amount;
            }
        }

        let def = self.attribute.definition();
        total.clamp(def.min, def.max)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mcrs_minecraft_resource::ResourceLocation;

    fn rl(name: &'static str) -> ResourceLocationBuf {
        ResourceLocation::read(name).unwrap().into()
    }

    #[test]
    fn attribute_base_and_modifiers_calculation() {
        let mut inst = AttributeInstance::new(Attribute::MaxHealth, 20.0);
        assert_eq!(inst.value(), 20.0);

        // Add 4 HP (AddValue)
        inst.add_modifier(AttributeModifier::new(rl("test:add"), 4.0, AttributeOperation::AddValue));
        assert_eq!(inst.value(), 24.0);

        // Add 50% of base (AddMultipliedBase): 24 + 24 * 0.5 = 36
        inst.add_modifier(AttributeModifier::new(rl("test:mult_base"), 0.5, AttributeOperation::AddMultipliedBase));
        assert_eq!(inst.value(), 36.0);

        // Multiply total by +100% (AddMultipliedTotal): 36 * 2.0 = 72
        inst.add_modifier(AttributeModifier::new(rl("test:mult_total"), 1.0, AttributeOperation::AddMultipliedTotal));
        assert_eq!(inst.value(), 72.0);
    }

    #[test]
    fn attribute_clamps_to_definition_limits() {
        // Armor has min 0.0, max 30.0
        let mut inst = AttributeInstance::new(Attribute::Armor, 10.0);
        inst.add_modifier(AttributeModifier::new(rl("test:huge"), 100.0, AttributeOperation::AddValue));
        assert_eq!(inst.value(), 30.0);

        inst.modifiers.clear();
        inst.add_modifier(AttributeModifier::new(rl("test:negative"), -50.0, AttributeOperation::AddValue));
        assert_eq!(inst.value(), 0.0);
    }
}
