use fixedbitset::FixedBitSet;
use mcrs_minecraft_entity_attribute::ModifierOperation;
use mcrs_minecraft_item_stack::AttributeModifier;
use mcrs_minecraft_item_stack::SlotGroup;
use mcrs_minecraft_registries::Registries;
use mcrs_minecraft_resource::ResourceLocationBuf;

use crate::id_bits::id_bits;
use crate::predicate_issue::PredicateIssue;
use mcrs_minecraft_item::component::common::EquipmentSlotGroup;
use mcrs_minecraft_item::component::common::MinMaxBounds;
use mcrs_minecraft_item::component::predicate::AttributeModifierPredicate;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct BakedModifierPredicate {
    attribute: Option<FixedBitSet>,
    id: Option<ResourceLocationBuf>,
    amount: MinMaxBounds<f64>,
    operation: Option<ModifierOperation>,
    slot: Option<SlotGroup>,
}

impl BakedModifierPredicate {
    pub(crate) fn bake(
        predicate: &AttributeModifierPredicate,
        registries: &Registries,
    ) -> Result<Self, PredicateIssue> {
        Ok(Self {
            attribute: predicate
                .attribute
                .as_ref()
                .map(|set| id_bits(set, registries, PredicateIssue::UnloadedAttributeTags))
                .transpose()?,
            id: predicate.id.clone(),
            amount: predicate.amount,
            operation: predicate.operation,
            slot: predicate.slot.map(slot_group),
        })
    }

    pub(crate) fn matches(&self, modifier: &AttributeModifier) -> bool {
        self.attribute
            .as_ref()
            .is_none_or(|bits| bits.contains(usize::from(modifier.attribute.0)))
            && self
                .id
                .as_ref()
                .is_none_or(|id| id.as_location() == modifier.id())
            && self.amount.matches(modifier.amount)
            && self
                .operation
                .is_none_or(|operation| operation == modifier.operation)
            && self.slot.is_none_or(|slot| slot == modifier.slot)
    }
}

fn slot_group(group: EquipmentSlotGroup) -> SlotGroup {
    match group {
        EquipmentSlotGroup::Any => SlotGroup::Any,
        EquipmentSlotGroup::Mainhand => SlotGroup::Mainhand,
        EquipmentSlotGroup::Offhand => SlotGroup::Offhand,
        EquipmentSlotGroup::Hand => SlotGroup::Hand,
        EquipmentSlotGroup::Feet => SlotGroup::Feet,
        EquipmentSlotGroup::Legs => SlotGroup::Legs,
        EquipmentSlotGroup::Chest => SlotGroup::Chest,
        EquipmentSlotGroup::Head => SlotGroup::Head,
        EquipmentSlotGroup::Armor => SlotGroup::Armor,
        EquipmentSlotGroup::Body => SlotGroup::Body,
        EquipmentSlotGroup::Saddle => SlotGroup::Saddle,
    }
}
