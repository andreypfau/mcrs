//! Attribute instances and the one fold of their modifiers (DMG-ATT-002).

use mcrs_minecraft_resource::ResourceLocationBuf;

use crate::attribute_keys::Attribute;

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
        Self {
            id,
            amount,
            operation,
        }
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
        self.modifiers.retain(|modifier| &modifier.id != id);
    }

    /// The value of the instance, clamped to the limits of its attribute.
    #[must_use]
    pub fn value(&self) -> f64 {
        let value = fold_attribute(
            self.base_value,
            self.modifiers
                .iter()
                .map(|modifier| (modifier.operation, modifier.amount)),
        );
        let definition = self.attribute.definition();
        if value.is_nan() {
            definition.min
        } else {
            value.clamp(definition.min, definition.max)
        }
    }
}

/// The base plus each added value, plus that sum times the sum of the base multipliers, then
/// times one plus each total multiplier. The caller clamps the result to the attribute limits.
#[must_use]
pub fn fold_attribute(
    base: f64,
    modifiers: impl IntoIterator<Item = (AttributeOperation, f64)>,
) -> f64 {
    let mut added = base;
    let mut multiplied_base = 0.0;
    let mut multiplied_total = 1.0;
    for (operation, amount) in modifiers {
        match operation {
            AttributeOperation::AddValue => added += amount,
            AttributeOperation::AddMultipliedBase => multiplied_base += amount,
            AttributeOperation::AddMultipliedTotal => multiplied_total *= 1.0 + amount,
        }
    }
    (added + added * multiplied_base) * multiplied_total
}

#[cfg(test)]
mod tests {
    use mcrs_minecraft_resource::ResourceLocation;

    use super::*;

    fn id(name: &'static str) -> ResourceLocationBuf {
        ResourceLocation::read(name).unwrap().into()
    }

    #[test]
    fn a_fold_adds_then_multiplies_the_base_and_the_total() {
        let mut instance = AttributeInstance::new(Attribute::MaxHealth, 20.0);
        instance.add_modifier(AttributeModifier::new(
            id("test:add"),
            4.0,
            AttributeOperation::AddValue,
        ));
        instance.add_modifier(AttributeModifier::new(
            id("test:base"),
            0.5,
            AttributeOperation::AddMultipliedBase,
        ));
        instance.add_modifier(AttributeModifier::new(
            id("test:total"),
            1.0,
            AttributeOperation::AddMultipliedTotal,
        ));
        assert_eq!(instance.value(), 72.0);
    }

    #[test]
    fn a_fold_clamps_to_the_limits_of_the_attribute() {
        let mut instance = AttributeInstance::new(Attribute::Armor, 10.0);
        instance.add_modifier(AttributeModifier::new(
            id("test:huge"),
            100.0,
            AttributeOperation::AddValue,
        ));
        assert_eq!(instance.value(), 30.0);
    }

    #[test]
    fn a_modifier_with_the_same_id_replaces_the_old_one() {
        let mut instance = AttributeInstance::new(Attribute::Armor, 0.0);
        instance.add_modifier(AttributeModifier::new(
            id("test:armor"),
            2.0,
            AttributeOperation::AddValue,
        ));
        instance.add_modifier(AttributeModifier::new(
            id("test:armor"),
            5.0,
            AttributeOperation::AddValue,
        ));
        assert_eq!(instance.value(), 5.0);
    }
}
