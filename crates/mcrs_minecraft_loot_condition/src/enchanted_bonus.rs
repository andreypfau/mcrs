use mcrs_minecraft_enchantment::level_based_value::LevelBasedValue;
use mcrs_minecraft_item::component::EquipmentSlotGroup;
use mcrs_minecraft_item::component::equippable::EquipmentSlot;
use mcrs_minecraft_item::enchantment::Enchantment;
use mcrs_minecraft_predicate::{EntityView, ItemSubject};
use mcrs_minecraft_resource::Id;

use crate::tool::Tool;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct EnchantedBonus {
    pub(crate) enchantment: Option<Id<Enchantment>>,
    pub(crate) slots: Box<[EquipmentSlot]>,
    pub(crate) unenchanted_chance: f32,
    pub(crate) enchanted_chance: LevelBasedValue,
}

impl EnchantedBonus {
    pub(crate) fn chance<W: EntityView>(&self, world: &W, attacker: Option<W::Entity>) -> f32 {
        let level = attacker
            .zip(self.enchantment)
            .map_or(0, |(entity, enchantment)| {
                equipment_level(world, entity, enchantment, &self.slots)
            });
        let level = i32::try_from(level).unwrap_or(i32::MAX);
        match level > 0 {
            true => self.enchanted_chance.calculate(level),
            false => self.unenchanted_chance,
        }
    }
}

#[must_use]
pub fn equipment_level<W: EntityView>(
    world: &W,
    entity: W::Entity,
    enchantment: Id<Enchantment>,
    slots: &[EquipmentSlot],
) -> u32 {
    if world.is_living(entity) != Some(true) {
        return 0;
    }
    slots
        .iter()
        .filter_map(|&slot| match world.equipment(entity, slot)? {
            ItemSubject::Stack(stack) => Some(Tool::Stack(stack).level(enchantment)),
            ItemSubject::Empty => None,
        })
        .max()
        .unwrap_or(0)
}

#[must_use]
pub fn equipment_slots(groups: &[EquipmentSlotGroup]) -> Box<[EquipmentSlot]> {
    EquipmentSlot::ALL
        .into_iter()
        .filter(|&slot| groups.iter().any(|&group| holds(group, slot)))
        .collect()
}

fn holds(group: EquipmentSlotGroup, slot: EquipmentSlot) -> bool {
    use EquipmentSlot as S;
    use EquipmentSlotGroup as G;
    match group {
        G::Any => true,
        G::Mainhand => slot == S::MainHand,
        G::Offhand => slot == S::OffHand,
        G::Hand => matches!(slot, S::MainHand | S::OffHand),
        G::Feet => slot == S::Feet,
        G::Legs => slot == S::Legs,
        G::Chest => slot == S::Chest,
        G::Head => slot == S::Head,
        G::Armor => matches!(slot, S::Feet | S::Legs | S::Chest | S::Head),
        G::Body => slot == S::Body,
        G::Saddle => slot == S::Saddle,
    }
}
