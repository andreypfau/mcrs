use mcrs_minecraft_inventory_player::ArmourSlot;
use mcrs_minecraft_inventory_player::EquipSlot;
use mcrs_minecraft_inventory_player::Holder;

use crate::MenuOrder;
use crate::MenuRange;
use crate::MenuSlot;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct InventoryMenu;

const CRAFT_RESULT: u16 = 0;
const HEAD: u16 = 5;
const CHEST: u16 = 6;
const LEGS: u16 = 7;
const FEET: u16 = 8;
const OFF_HAND: u16 = 45;

const fn ascending(start: u16, end: u16) -> MenuRange {
    MenuRange::new(start, end, MenuOrder::Ascending)
}

const fn only(index: u16) -> MenuRange {
    ascending(index, index + 1)
}

const FROM_CRAFT_RESULT: [MenuRange; 1] = [MenuRange::new(9, 45, MenuOrder::Descending)];
const FROM_GRID_AND_ARMOUR: [MenuRange; 1] = [ascending(9, 45)];
const TO_HEAD: [MenuRange; 1] = [only(HEAD)];
const TO_CHEST: [MenuRange; 1] = [only(CHEST)];
const TO_LEGS: [MenuRange; 1] = [only(LEGS)];
const TO_FEET: [MenuRange; 1] = [only(FEET)];
const TO_OFF_HAND: [MenuRange; 1] = [only(OFF_HAND)];
const TO_HOTBAR: [MenuRange; 1] = [ascending(36, 45)];
const TO_MAIN: [MenuRange; 1] = [ascending(9, 36)];
const FROM_OFF_HAND: [MenuRange; 1] = [ascending(9, 45)];

impl InventoryMenu {
    pub const CONTAINER_ID: u8 = 0;
    pub const LEN: u16 = 46;

    #[must_use]
    pub fn slot(index: u16) -> Option<MenuSlot> {
        let slot = match index {
            CRAFT_RESULT => MenuSlot::new(Holder::CraftResult, 0),
            1..=4 => MenuSlot::new(Holder::CraftGrid, index - 1),
            HEAD..=FEET => MenuSlot::new(Holder::Inventory, 44 - index),
            9..=35 => MenuSlot::new(Holder::Inventory, index),
            36..=44 => MenuSlot::new(Holder::Inventory, index - 36),
            OFF_HAND => MenuSlot::new(Holder::Inventory, 40),
            _ => return None,
        };
        Some(slot)
    }

    #[must_use]
    pub fn index(slot: MenuSlot) -> Option<u16> {
        match (slot.holder, slot.index) {
            (Holder::CraftResult, 0) => Some(CRAFT_RESULT),
            (Holder::CraftGrid, index @ 0..=3) => Some(index + 1),
            (Holder::Inventory, index @ 0..=8) => Some(index + 36),
            (Holder::Inventory, index @ 9..=35) => Some(index),
            (Holder::Inventory, index @ 36..=39) => Some(44 - index),
            (Holder::Inventory, 40) => Some(OFF_HAND),
            _ => None,
        }
    }

    #[must_use]
    pub fn quick_move_destinations(
        source: u16,
        equip: Option<EquipSlot>,
        holds_empty: impl FnOnce(u16) -> bool,
    ) -> Option<&'static [MenuRange]> {
        match source {
            CRAFT_RESULT => return Some(&FROM_CRAFT_RESULT),
            1..=FEET => return Some(&FROM_GRID_AND_ARMOUR),
            _ if source >= Self::LEN => return None,
            _ => {}
        }
        if let Some(equip) = equip {
            let (target, destinations): (u16, &'static [MenuRange]) = match equip {
                EquipSlot::Armour(ArmourSlot::Head) => (HEAD, &TO_HEAD),
                EquipSlot::Armour(ArmourSlot::Chest) => (CHEST, &TO_CHEST),
                EquipSlot::Armour(ArmourSlot::Legs) => (LEGS, &TO_LEGS),
                EquipSlot::Armour(ArmourSlot::Feet) => (FEET, &TO_FEET),
                EquipSlot::OffHand => (OFF_HAND, &TO_OFF_HAND),
            };
            if holds_empty(target) {
                return Some(destinations);
            }
        }
        let destinations: &'static [MenuRange] = match source {
            9..=35 => &TO_HOTBAR,
            36..=44 => &TO_MAIN,
            OFF_HAND => &FROM_OFF_HAND,
            _ => &[],
        };
        Some(destinations)
    }

    #[must_use]
    pub fn permits_pick_up_all(index: u16) -> bool {
        index != CRAFT_RESULT && index < Self::LEN
    }

    #[must_use]
    pub fn permits_drag(index: u16) -> bool {
        index < Self::LEN
    }
}
