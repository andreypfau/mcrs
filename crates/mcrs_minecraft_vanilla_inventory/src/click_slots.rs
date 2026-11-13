use bevy_ecs::entity::Entity;
use bevy_ecs::system::Query;
use mcrs_minecraft_engine_item::SlotRef;
use mcrs_minecraft_engine_item::Slots;
use mcrs_minecraft_engine_item::Stacks;
use mcrs_minecraft_inventory_plan::Filter;
use mcrs_minecraft_inventory_plan::Loaded;
use mcrs_minecraft_inventory_plan::Planned;
use mcrs_minecraft_item_stack::Damage;
use mcrs_minecraft_item_stack::MaxStackSize;
use mcrs_minecraft_menu::MenuSlot;
use mcrs_minecraft_player_slots::ArmourLock;
use mcrs_minecraft_player_slots::Holder;
use mcrs_minecraft_player_slots::OffHandRule;
use mcrs_minecraft_player_slots::Offer;
use mcrs_minecraft_player_slots::PlayerMode;
use mcrs_minecraft_player_slots::Stacking;

use crate::PlayerHolders;

const HOLDERS: [Holder; 4] = [
    Holder::Inventory,
    Holder::Cursor,
    Holder::CraftGrid,
    Holder::CraftResult,
];
const SLOT_COUNT: usize = 47;
const LARGEST_STACK: i32 = 99;

/// A stack of a click: its entity, the first slot that holds a stack with the same item and
/// components, and whether it can stack beside its maximum stack size.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct StackHandle {
    pub(crate) entity: Entity,
    alike: usize,
    stacking: Stacking,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SlotState {
    Missing,
    Holds(Option<Planned<StackHandle>>),
}

/// The four holders of a player as one click reads them, taken before the click.
// chisle: no stack offers a `minecraft:equippable` slot or an armour lock, because item stacks do
// not hold these components yet; the armour slots refuse each place and permit each take until
// they do.
pub(crate) struct ClickSlots {
    player: Entity,
    holders: PlayerHolders,
    slots: [SlotState; SLOT_COUNT],
    mode: PlayerMode,
}

impl ClickSlots {
    pub(crate) fn read(
        player: Entity,
        inventory: &Slots,
        holders: PlayerHolders,
        holder_slots: &Query<&Slots>,
        mode: PlayerMode,
        stacks: &mut Stacks,
    ) -> Self {
        let mut slots = [SlotState::Missing; SLOT_COUNT];
        for holder in HOLDERS {
            let content = match holder {
                Holder::Inventory => Some(inventory),
                Holder::Cursor => holder_slots.get(holders.cursor).ok(),
                Holder::CraftGrid => holder_slots.get(holders.craft_grid).ok(),
                Holder::CraftResult => holder_slots.get(holders.craft_result).ok(),
            };
            let Some(content) = content else {
                continue;
            };
            for index in 0..holder.slot_count() {
                let Some(flat) = flat(MenuSlot::new(holder, index)) else {
                    continue;
                };
                if usize::from(index) >= content.len() {
                    continue;
                }
                let state = match content.get(usize::from(index)) {
                    Some(entity) => SlotState::Holds(planned(entity, flat, &slots, stacks)),
                    None => SlotState::Holds(None),
                };
                if let Some(slot) = slots.get_mut(flat) {
                    *slot = state;
                }
            }
        }
        Self {
            player,
            holders,
            slots,
            mode,
        }
    }

    pub(crate) fn slot_ref(&self, slot: MenuSlot) -> SlotRef {
        let holder = match slot.holder {
            Holder::Inventory => self.player,
            Holder::Cursor => self.holders.cursor,
            Holder::CraftGrid => self.holders.craft_grid,
            Holder::CraftResult => self.holders.craft_result,
        };
        SlotRef {
            holder,
            index: slot.index,
        }
    }

    pub(crate) fn holds_empty(&self, slot: MenuSlot) -> bool {
        self.state(slot) == SlotState::Holds(None)
    }

    fn state(&self, slot: MenuSlot) -> SlotState {
        flat(slot)
            .and_then(|flat| self.slots.get(flat).copied())
            .unwrap_or(SlotState::Missing)
    }
}

impl mcrs_minecraft_inventory_click::Slots for ClickSlots {
    type Slot = MenuSlot;
    type Stack = StackHandle;

    fn cursor(&self) -> MenuSlot {
        MenuSlot::new(Holder::Cursor, 0)
    }

    fn inventory(&self, index: u16) -> Option<MenuSlot> {
        (index < Holder::Inventory.slot_count()).then_some(MenuSlot::new(Holder::Inventory, index))
    }

    fn load(&self, slot: MenuSlot) -> Option<Loaded<StackHandle>> {
        let rule = slot.holder.slot(slot.index)?;
        let SlotState::Holds(stack) = self.state(slot) else {
            return None;
        };
        Some(Loaded {
            stack,
            limit: rule.limit,
            take: rule.filter.take(ArmourLock::Free, self.mode),
        })
    }

    fn place(&self, slot: MenuSlot, _stack: StackHandle) -> Filter {
        match slot.holder.slot(slot.index) {
            Some(rule) => rule.filter.place(Offer::default(), OffHandRule::AnyItem),
            None => Filter::Refuses,
        }
    }

    fn equal(&self, left: StackHandle, right: StackHandle) -> bool {
        left.alike == right.alike
    }

    fn can_stack(&self, stack: Planned<StackHandle>) -> bool {
        stack.max > 1 && stack.stack.stacking == Stacking::Stacks
    }
}

/// The place of `slot` in the snapshot: the player inventory, the cursor, the craft grid, then
/// the craft result.
fn flat(slot: MenuSlot) -> Option<usize> {
    let mut base = 0;
    for holder in HOLDERS {
        if holder == slot.holder {
            return (slot.index < holder.slot_count()).then(|| base + usize::from(slot.index));
        }
        base += usize::from(holder.slot_count());
    }
    None
}

/// `slots` holds the stacks read so far; the first of them that is alike gives its `alike`, and
/// a stack without one gives `flat`, its own place.
fn planned(
    entity: Entity,
    flat: usize,
    slots: &[SlotState],
    stacks: &mut Stacks,
) -> Option<Planned<StackHandle>> {
    let kind = stacks.kind(entity)?;
    let count = stacks.count(entity)?;
    let max = (stacks.get::<MaxStackSize>(entity)).map_or(1, |max| max.0.clamp(1, LARGEST_STACK));
    let stacking = if stacks
        .get::<Damage>(entity)
        .is_some_and(|damage| damage.0 > 0)
    {
        Stacking::Alone
    } else {
        Stacking::Stacks
    };
    let alike = (slots.iter())
        .filter_map(|state| match state {
            SlotState::Holds(Some(stack)) => Some(stack.stack),
            _ => None,
        })
        .find(|other| stacks.mergeable(other.entity, entity))
        .map_or(flat, |other| other.alike);
    Some(Planned {
        stack: StackHandle {
            entity,
            alike,
            stacking,
        },
        kind: kind.0,
        count: count.get(),
        max: u8::try_from(max).unwrap_or(1),
    })
}
