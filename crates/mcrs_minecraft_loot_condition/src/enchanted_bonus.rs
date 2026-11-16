use mcrs_minecraft_enchantment::level_based_value::LevelBasedValue;
use mcrs_minecraft_item::component::EquipmentSlotGroup;
use mcrs_minecraft_item::component::equippable::EquipmentSlot;
use mcrs_minecraft_item::enchantment::Enchantment;
use mcrs_minecraft_predicate::{EntityView, ItemSubject};
use mcrs_minecraft_resource::Id;

use crate::tool::Tool;

/// The fields of `random_chance_with_enchanted_bonus`, with the equipment slots that the field
/// `slots` of the enchantment names (LOOT-CND-010, LOOT-CND-022).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct EnchantedBonus {
    pub(crate) enchantment: Option<Id<Enchantment>>,
    pub(crate) slots: Box<[EquipmentSlot]>,
    pub(crate) unenchanted_chance: f32,
    pub(crate) enchanted_chance: LevelBasedValue,
}

impl EnchantedBonus {
    /// The chance for the attacking entity `attacker` (LOOT-CND-010).
    pub(crate) fn chance<W: EntityView>(&self, world: &W, attacker: Option<W::Entity>) -> f32 {
        let level = attacker
            .zip(self.enchantment)
            .map_or(0, |(entity, enchantment)| {
                self.level(world, entity, enchantment)
            });
        let level = i32::try_from(level).unwrap_or(i32::MAX);
        match level > 0 {
            true => self.enchanted_chance.calculate(level),
            false => self.unenchanted_chance,
        }
    }

    /// The level of `enchantment` on `entity` (LOOT-CND-022); 0 for an entity that is not living.
    fn level<W: EntityView>(
        &self,
        world: &W,
        entity: W::Entity,
        enchantment: Id<Enchantment>,
    ) -> u32 {
        if world.is_living(entity) != Some(true) {
            return 0;
        }
        self.slots
            .iter()
            .filter_map(|&slot| match world.equipment(entity, slot)? {
                ItemSubject::Stack(stack) => Some(Tool::Stack(stack).level(enchantment)),
                ItemSubject::Empty => None,
            })
            .max()
            .unwrap_or(0)
    }
}

/// The equipment slots of the slot groups `groups`, in the order of [`EquipmentSlot::ALL`].
pub(crate) fn equipment_slots(groups: &[EquipmentSlotGroup]) -> Box<[EquipmentSlot]> {
    EquipmentSlot::ALL
        .into_iter()
        .filter(|&slot| groups.iter().any(|&group| holds(group, slot)))
        .collect()
}

// chisle: LOOT-CND-022 does not state the slots of each group (`armor`, `any`); the specification author answers it.
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
