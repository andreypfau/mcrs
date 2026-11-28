use std::borrow::Cow;

use mcrs_minecraft_entity_attribute::ModifierOperation;
use mcrs_minecraft_item_stack::{AttributeId, AttributeModifier, AttributeModifiers, SlotGroup};
use mcrs_minecraft_loot_condition::{FloatProvider, LootWorld, bounded};
use mcrs_minecraft_predicate::PredicateScratch;
use mcrs_minecraft_random::Random;
use mcrs_minecraft_resource::ResourceLocationBuf;

use crate::table::Evaluation;
use crate::work_stack::WorkStack;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SetAttribute {
    pub(crate) attribute: AttributeId,
    pub(crate) id: ResourceLocationBuf,
    pub(crate) amount: FloatProvider,
    pub(crate) operation: ModifierOperation,
    pub(crate) slots: Box<[SlotGroup]>,
}

pub(crate) fn set_attributes<E: Copy, W: LootWorld<E>, R: Random>(
    stack: &mut WorkStack,
    elements: &[SetAttribute],
    replace: bool,
    evaluation: &Evaluation<'_, '_, E, W>,
    random: &mut R,
    scratch: &mut PredicateScratch,
) {
    let items = &evaluation.rules.items;
    let mut list = match (replace, stack.read()) {
        (false, Some(read)) => items
            .effective::<AttributeModifiers>(read)
            .map(|modifiers| modifiers.modifiers.to_vec())
            .unwrap_or_default(),
        _ => Vec::new(),
    };
    for element in elements {
        let length = i64::try_from(element.slots.len()).unwrap_or(i64::MAX);
        let slot = bounded(random, length)
            .and_then(|index| usize::try_from(index).ok())
            .and_then(|index| element.slots.get(index).copied())
            .unwrap_or_default();
        let amount = element.amount.read(evaluation.inputs(), random, scratch);
        list.retain(|modifier| {
            modifier.attribute != element.attribute || modifier.id() != element.id.as_location()
        });
        let mut modifier = AttributeModifier::new(
            element.attribute,
            element.id.clone(),
            f64::from(amount),
            element.operation,
        );
        modifier.slot = slot;
        list.push(modifier);
    }
    let modifiers = AttributeModifiers {
        modifiers: Cow::Owned(list),
    };
    stack.set(modifiers, items);
}
