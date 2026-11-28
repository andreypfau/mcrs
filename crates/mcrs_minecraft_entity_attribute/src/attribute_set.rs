use alloc::vec::Vec;
use core::mem::size_of;

use mcrs_minecraft_resource::ResourceLocation;

use crate::attribute_bases::AttributeBases;
use crate::attribute_error::AttributeError;
use crate::attribute_keys::Attribute;
use crate::fold::fold;
use crate::instance::Instance;
use crate::instance_change::InstanceChange;
use crate::instance_modifier::InstanceModifier;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct AttributeSet {
    instances: Vec<Instance>,
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

    #[must_use]
    pub fn base(&self, bases: AttributeBases, attribute: Attribute) -> Option<f64> {
        let type_base = bases.base(attribute)?;
        Some(
            self.instance(attribute)
                .map_or(type_base, |instance| instance.base),
        )
    }

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

    pub fn instances(&self) -> impl Iterator<Item = &Instance> {
        self.instances.iter()
    }

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

    #[must_use]
    pub fn instance(&self, attribute: Attribute) -> Option<&Instance> {
        self.instances
            .iter()
            .find(|instance| instance.attribute == attribute)
    }

    fn change<R>(
        &mut self,
        bases: AttributeBases,
        attribute: Attribute,
        edit: impl FnOnce(&mut Instance) -> R,
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
        let mut instance = Instance::new_of_type(attribute, type_base);
        let result = edit(&mut instance);
        if !instance.is_new_of_type(type_base) {
            self.insert(instance);
        }
        Ok(result)
    }

    fn insert(&mut self, instance: Instance) {
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

fn has_modifier(instance: &Instance, id: ResourceLocation<'_>) -> bool {
    instance.modifiers.iter().any(|present| present.id() == id)
}
