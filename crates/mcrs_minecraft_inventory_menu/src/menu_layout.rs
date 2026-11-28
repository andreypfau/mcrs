use alloc::borrow::Cow;
use alloc::vec::Vec;

use mcrs_minecraft_inventory_player::ArmourSlot;
use mcrs_minecraft_inventory_player::EquipSlot;
use mcrs_minecraft_menu_key::menu_keys::MenuType;

use crate::MenuEntry;
use crate::MenuOrder;
use crate::MenuRange;
use crate::Route;
use crate::RouteWhen;

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "bevy", derive(bevy_ecs::component::Component))]
pub struct MenuLayout {
    kind: Option<MenuType>,
    entries: Cow<'static, [MenuEntry]>,
    routes: Cow<'static, [Route]>,
    pick_up_all: u128,
}

const PLAYER: u8 = 0;
const CRAFT_GRID: u8 = 1;
const CRAFT_RESULT: u8 = 2;
const PLAYER_SLOTS: u16 = 36;
const HOTBAR_SLOTS: u16 = 9;

const fn player(index: u16) -> MenuEntry {
    MenuEntry {
        part: PLAYER,
        index,
    }
}

const fn grid(index: u16) -> MenuEntry {
    MenuEntry {
        part: CRAFT_GRID,
        index,
    }
}

const INVENTORY_ENTRIES: [MenuEntry; 46] = [
    MenuEntry {
        part: CRAFT_RESULT,
        index: 0,
    },
    grid(0),
    grid(1),
    grid(2),
    grid(3),
    player(39),
    player(38),
    player(37),
    player(36),
    player(9),
    player(10),
    player(11),
    player(12),
    player(13),
    player(14),
    player(15),
    player(16),
    player(17),
    player(18),
    player(19),
    player(20),
    player(21),
    player(22),
    player(23),
    player(24),
    player(25),
    player(26),
    player(27),
    player(28),
    player(29),
    player(30),
    player(31),
    player(32),
    player(33),
    player(34),
    player(35),
    player(0),
    player(1),
    player(2),
    player(3),
    player(4),
    player(5),
    player(6),
    player(7),
    player(8),
    player(40),
];

const fn up(start: u16, end: u16) -> MenuRange {
    MenuRange::new(start, end, MenuOrder::Ascending)
}

const fn always(from: MenuRange, to: MenuRange) -> Route {
    Route {
        from,
        when: RouteWhen::Always,
        to,
    }
}

const fn wears(slot: EquipSlot, target: u16) -> Route {
    Route {
        from: up(9, 46),
        when: RouteWhen::Wears { slot, target },
        to: up(target, target + 1),
    }
}

const INVENTORY_ROUTES: [Route; 10] = [
    always(up(0, 1), MenuRange::new(9, 45, MenuOrder::Descending)),
    always(up(1, 9), up(9, 45)),
    wears(EquipSlot::Armour(ArmourSlot::Head), 5),
    wears(EquipSlot::Armour(ArmourSlot::Chest), 6),
    wears(EquipSlot::Armour(ArmourSlot::Legs), 7),
    wears(EquipSlot::Armour(ArmourSlot::Feet), 8),
    wears(EquipSlot::OffHand, 45),
    always(up(9, 36), up(36, 45)),
    always(up(36, 45), up(9, 36)),
    always(up(45, 46), up(9, 45)),
];

const INVENTORY_MASK: u128 = (1 << INVENTORY_ENTRIES.len()) - 1;

fn mask(len: u16) -> u128 {
    MenuLayout::MAX_LEN
        .checked_sub(len)
        .and_then(|unused| u128::MAX.checked_shr(u32::from(unused)))
        .unwrap_or(0)
}

fn bit(mask: u128, index: u16) -> bool {
    mask.checked_shr(u32::from(index))
        .is_some_and(|shifted| shifted & 1 == 1)
}

fn fits(range: MenuRange, len: u16) -> bool {
    range.start <= range.end && range.end <= len
}

impl MenuLayout {
    pub const GENERIC_CONTAINER: u8 = 0;
    pub const GENERIC_PLAYER: u8 = 1;

    pub const INVENTORY: MenuLayout = MenuLayout {
        kind: None,
        entries: Cow::Borrowed(&INVENTORY_ENTRIES),
        routes: Cow::Borrowed(&INVENTORY_ROUTES),
        pick_up_all: INVENTORY_MASK & !1,
    };
    pub const MAX_LEN: u16 = 128;

    #[must_use]
    pub fn new(kind: MenuType, entries: Vec<MenuEntry>, routes: Vec<Route>) -> Option<Self> {
        let len = u16::try_from(entries.len())
            .ok()
            .filter(|len| *len <= Self::MAX_LEN)?;
        let valid = routes.iter().all(|route| {
            let target_fits = match route.when {
                RouteWhen::Always => true,
                RouteWhen::Wears { target, .. } => target < len,
            };
            fits(route.from, len) && fits(route.to, len) && target_fits
        });
        if !valid {
            return None;
        }
        Some(Self {
            kind: Some(kind),
            entries: Cow::Owned(entries),
            routes: Cow::Owned(routes),
            pick_up_all: mask(len),
        })
    }

    #[must_use]
    pub fn of(kind: MenuType) -> Option<Self> {
        let container = match kind {
            MenuType::Generic9x1 | MenuType::Generic3x3 => 9,
            MenuType::Generic9x2 => 18,
            MenuType::Generic9x3 => 27,
            MenuType::Generic9x4 => 36,
            MenuType::Generic9x5 => 45,
            MenuType::Generic9x6 => 54,
            MenuType::Hopper => 5,
            _ => return None,
        };
        Some(Self::generic(kind, container))
    }

    fn generic(kind: MenuType, container: u16) -> Self {
        let len = container + PLAYER_SLOTS;
        let entries = (0..container)
            .map(|index| MenuEntry {
                part: Self::GENERIC_CONTAINER,
                index,
            })
            .chain(
                (HOTBAR_SLOTS..PLAYER_SLOTS)
                    .chain(0..HOTBAR_SLOTS)
                    .map(|index| MenuEntry {
                        part: Self::GENERIC_PLAYER,
                        index,
                    }),
            )
            .collect();
        let routes = Vec::from([
            always(
                up(0, container),
                MenuRange::new(container, len, MenuOrder::Descending),
            ),
            always(up(container, len), up(0, container)),
        ]);
        Self {
            kind: Some(kind),
            entries: Cow::Owned(entries),
            routes: Cow::Owned(routes),
            pick_up_all: mask(len),
        }
    }

    #[must_use]
    pub fn part_len(&self, part: u8) -> u16 {
        self.entries
            .iter()
            .filter(|entry| entry.part == part)
            .map(|entry| entry.index.saturating_add(1))
            .max()
            .unwrap_or(0)
    }

    #[must_use]
    pub fn len(&self) -> u16 {
        u16::try_from(self.entries.len()).unwrap_or(u16::MAX)
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    #[must_use]
    pub fn entry(&self, index: u16) -> Option<MenuEntry> {
        self.entries.get(usize::from(index)).copied()
    }

    #[must_use]
    pub fn index_of(&self, entry: MenuEntry) -> Option<u16> {
        let position = self
            .entries
            .iter()
            .position(|candidate| *candidate == entry)?;
        u16::try_from(position).ok()
    }

    pub fn routes(&self, source: u16) -> impl Iterator<Item = Route> + '_ {
        self.routes
            .iter()
            .copied()
            .filter(move |route| route.from.start <= source && source < route.from.end)
    }

    #[must_use]
    pub fn permits_pick_up_all(&self, index: u16) -> bool {
        bit(self.pick_up_all, index)
    }

    #[must_use]
    pub fn permits_drag(&self, index: u16) -> bool {
        index < self.len()
    }

    #[must_use]
    pub fn kind(&self) -> Option<MenuType> {
        self.kind
    }
}
