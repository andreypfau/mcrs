use bevy_ecs::prelude::Component;

/// Slots of a player: the vanilla inventory menu order plus the cursor.
pub mod slots {
    use std::ops::Range;

    pub const RESULT: u16 = 0;
    pub const CRAFT: Range<u16> = 1..5;
    pub const ARMOR_HEAD: u16 = 5;
    pub const ARMOR_CHEST: u16 = 6;
    pub const ARMOR_LEGS: u16 = 7;
    pub const ARMOR_FEET: u16 = 8;
    pub const MAIN: Range<u16> = 9..36;
    pub const HOTBAR: Range<u16> = 36..45;
    pub const OFFHAND: u16 = 45;
    pub const CARRIED: u16 = 46;
    pub const COUNT: usize = 47;
    pub const MENU_COUNT: usize = 46;

    const INVENTORY_FEET: u8 = 36;
    const INVENTORY_LEGS: u8 = 37;
    const INVENTORY_CHEST: u8 = 38;
    const INVENTORY_HEAD: u8 = 39;
    const INVENTORY_OFFHAND: u8 = 40;

    pub const fn held(selected: u8) -> u16 {
        HOTBAR.start + selected as u16
    }

    /// The vanilla `Inventory` index of a menu slot: hotbar 0..9, main 9..36,
    /// armour feet to head 36..40, offhand 40.
    pub const fn inventory_index(menu: u16) -> Option<u8> {
        match menu {
            ARMOR_HEAD => Some(INVENTORY_HEAD),
            ARMOR_CHEST => Some(INVENTORY_CHEST),
            ARMOR_LEGS => Some(INVENTORY_LEGS),
            ARMOR_FEET => Some(INVENTORY_FEET),
            OFFHAND => Some(INVENTORY_OFFHAND),
            _ if menu >= MAIN.start && menu < MAIN.end => Some(menu as u8),
            _ if menu >= HOTBAR.start && menu < HOTBAR.end => Some((menu - HOTBAR.start) as u8),
            _ => None,
        }
    }

    pub const fn from_inventory_index(inv: u8) -> Option<u16> {
        match inv {
            0..=8 => Some(HOTBAR.start + inv as u16),
            9..=35 => Some(inv as u16),
            INVENTORY_FEET => Some(ARMOR_FEET),
            INVENTORY_LEGS => Some(ARMOR_LEGS),
            INVENTORY_CHEST => Some(ARMOR_CHEST),
            INVENTORY_HEAD => Some(ARMOR_HEAD),
            INVENTORY_OFFHAND => Some(OFFHAND),
            _ => None,
        }
    }
}

#[derive(Component, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectedHotbarSlot(pub u8);

#[cfg(test)]
mod tests {
    use super::slots;

    #[test]
    fn menu_and_inventory_indices_are_inverses() {
        for menu in 0..slots::COUNT as u16 {
            match slots::inventory_index(menu) {
                Some(inv) => assert_eq!(slots::from_inventory_index(inv), Some(menu), "{menu}"),
                None => assert!(
                    menu == slots::RESULT || slots::CRAFT.contains(&menu) || menu == slots::CARRIED
                ),
            }
        }
        for inv in 0..=40u8 {
            let menu = slots::from_inventory_index(inv).unwrap();
            assert_eq!(slots::inventory_index(menu), Some(inv));
        }
        assert_eq!(slots::from_inventory_index(41), None);
        assert_eq!(slots::held(3), 39);
    }
}
