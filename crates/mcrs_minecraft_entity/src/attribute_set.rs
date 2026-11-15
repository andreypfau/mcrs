use core::mem::size_of;

use mcrs_minecraft_entity_attribute::fold;
use mcrs_minecraft_resource::ResourceLocation;

use crate::attribute_bases::AttributeBases;
use crate::attribute_error::AttributeError;
use crate::attribute_instance::AttributeInstance;
use crate::instance_change::InstanceChange;
use crate::instance_modifier::InstanceModifier;
use mcrs_minecraft_entity_attribute::attribute_keys::Attribute;

/// The attribute instances of one entity that differ from those of a new entity of its type
/// (`attributes.md`, section 3). An attribute of the set of the type with no entry has the base
/// of the type and no modifier. Each call takes the bases of the type of the entity.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AttributeSet {
    /// In the order of the numeric ids of the attributes.
    instances: Vec<AttributeInstance>,
}

const _: () = assert!(
    size_of::<AttributeSet>() == 3 * size_of::<usize>(),
    "an empty set is an empty Vec and holds no heap"
);

impl AttributeSet {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            instances: Vec::new(),
        }
    }

    /// The base value, or `None` when the attribute is not in the set of the type.
    #[must_use]
    pub fn base(&self, bases: AttributeBases, attribute: Attribute) -> Option<f64> {
        let type_base = bases.base(attribute)?;
        Some(
            self.instance(attribute)
                .map_or(type_base, |instance| instance.base),
        )
    }

    /// The value of ATR-VAL-001, or `None` when the attribute is not in the set of the type.
    #[must_use]
    pub fn value(&self, bases: AttributeBases, attribute: Attribute) -> Option<f64> {
        let type_base = bases.base(attribute)?;
        let range = attribute.definition().range;
        Some(match self.instance(attribute) {
            None => range.clamp(type_base),
            Some(instance) => range.clamp(fold(
                instance.base,
                instance.modifiers.iter().map(InstanceModifier::to_modifier),
            )),
        })
    }

    /// The instances that differ from a new entity of the type, in the order of the numeric ids.
    pub fn instances(&self) -> impl Iterator<Item = &AttributeInstance> {
        self.instances.iter()
    }

    /// Stores `base` as given, outside the range too (ATR-INS-002).
    pub fn set_base(
        &mut self,
        bases: AttributeBases,
        attribute: Attribute,
        base: f64,
    ) -> Result<InstanceChange, AttributeError> {
        self.change(bases, attribute, |instance| {
            if instance.base.to_bits() == base.to_bits() {
                InstanceChange::Unchanged
            } else {
                instance.base = base;
                InstanceChange::Changed
            }
        })
    }

    /// Puts `modifier` at the end of the sequence; an error when its identifier is present
    /// (ATR-INS-004).
    pub fn add_modifier(
        &mut self,
        bases: AttributeBases,
        attribute: Attribute,
        modifier: InstanceModifier,
    ) -> Result<(), AttributeError> {
        self.change(bases, attribute, |instance| {
            if has_modifier(instance, modifier.id()) {
                return Err(AttributeError::ModifierPresent);
            }
            instance.modifiers.push(modifier);
            Ok(())
        })?
    }

    /// Leaves an equal modifier in place; puts any other at the end of the sequence in place of
    /// the modifier of its identifier (ATR-INS-004).
    pub fn set_modifier(
        &mut self,
        bases: AttributeBases,
        attribute: Attribute,
        modifier: InstanceModifier,
    ) -> Result<InstanceChange, AttributeError> {
        self.change(bases, attribute, |instance| {
            let present = instance
                .modifiers
                .iter()
                .find(|present| present.id() == modifier.id());
            if present.is_some_and(|present| present.same_as(&modifier)) {
                return InstanceChange::Unchanged;
            }
            instance
                .modifiers
                .retain(|present| present.id() != modifier.id());
            instance.modifiers.push(modifier);
            InstanceChange::Changed
        })
    }

    /// Removes the modifier of `id`; `Unchanged` when the instance has none (ATR-INS-004).
    pub fn remove_modifier(
        &mut self,
        bases: AttributeBases,
        attribute: Attribute,
        id: ResourceLocation<'_>,
    ) -> Result<InstanceChange, AttributeError> {
        self.change(bases, attribute, |instance| {
            if !has_modifier(instance, id) {
                return InstanceChange::Unchanged;
            }
            instance.modifiers.retain(|present| present.id() != id);
            InstanceChange::Changed
        })
    }

    /// Removes every modifier of the instance and keeps its base; `Unchanged` when it has none.
    pub fn clear_modifiers(
        &mut self,
        bases: AttributeBases,
        attribute: Attribute,
    ) -> Result<InstanceChange, AttributeError> {
        self.change(bases, attribute, |instance| {
            if instance.modifiers.is_empty() {
                return InstanceChange::Unchanged;
            }
            instance.modifiers.clear();
            InstanceChange::Changed
        })
    }

    /// The instance of `attribute`, or `None` when it has the base of the type and no modifier.
    #[must_use]
    pub fn instance(&self, attribute: Attribute) -> Option<&AttributeInstance> {
        self.instances
            .iter()
            .find(|instance| instance.attribute == attribute)
    }

    /// Applies `edit` to the instance of `attribute`, and keeps an entry only for an instance
    /// that differs from a new one of the type.
    fn change<R>(
        &mut self,
        bases: AttributeBases,
        attribute: Attribute,
        edit: impl FnOnce(&mut AttributeInstance) -> R,
    ) -> Result<R, AttributeError> {
        let type_base = bases.base(attribute).ok_or(AttributeError::NotInSet)?;
        if let Some(instance) = self
            .instances
            .iter_mut()
            .find(|instance| instance.attribute == attribute)
        {
            let result = edit(instance);
            self.instances.retain(|instance| {
                instance.attribute != attribute || !instance.is_new_of_type(type_base)
            });
            return Ok(result);
        }
        let mut instance = AttributeInstance::new_of_type(attribute, type_base);
        let result = edit(&mut instance);
        if !instance.is_new_of_type(type_base) {
            self.insert(instance);
        }
        Ok(result)
    }

    fn insert(&mut self, instance: AttributeInstance) {
        let number = instance.attribute.id().number();
        let index = self
            .instances
            .partition_point(|present| present.attribute.id().number() < number);
        self.instances.push(instance);
        if let Some(tail) = self.instances.get_mut(index..) {
            tail.rotate_right(1);
        }
    }
}

fn has_modifier(instance: &AttributeInstance, id: ResourceLocation<'_>) -> bool {
    instance.modifiers.iter().any(|present| present.id() == id)
}
