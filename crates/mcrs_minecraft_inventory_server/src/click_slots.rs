use bevy_ecs::entity::Entity;
use bevy_ecs::system::Query;
use bevy_ecs::world::World;
use mcrs_minecraft_engine_item::Gate;
use mcrs_minecraft_engine_item::GateQuery;
use mcrs_minecraft_engine_item::SlotAccess;
use mcrs_minecraft_engine_item::SlotPolicy;
use mcrs_minecraft_engine_item::SlotRef;
use mcrs_minecraft_engine_item::Slots;
use mcrs_minecraft_engine_item::Stacks;
use mcrs_minecraft_inventory_menu::MenuSlot;
use mcrs_minecraft_inventory_player::Holder;
use mcrs_minecraft_inventory_player::Stacking;
use mcrs_minecraft_inventory_transaction::Filter;
use mcrs_minecraft_inventory_transaction::Loaded;
use mcrs_minecraft_inventory_transaction::Planned;
use mcrs_minecraft_item_stack::Damage;
use mcrs_minecraft_item_stack::MaxStackSize;

use crate::holder_slots::holder_entity;
use crate::stack_size::clamp_stack_size;
use mcrs_minecraft_vanilla_inventory::PlayerHolders;

const HOLDERS: [Holder; 4] = [
    Holder::Inventory,
    Holder::Cursor,
    Holder::CraftGrid,
    Holder::CraftResult,
];
const SLOT_COUNT: usize = 47;

/// The access of a slot of a holder without a [`SlotPolicy`], as the commit reads it.
const OPEN: SlotAccess = SlotAccess {
    limit: 99,
    take: Gate::Permits,
    place: Gate::Permits,
};

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
    Holds {
        stack: Option<Planned<StackHandle>>,
        access: SlotAccess,
    },
}

/// The four holders of a player as one click reads them, taken before the click, with the
/// [`SlotPolicy`] of each holder that the commit reads.
pub(crate) struct ClickSlots<'w> {
    world: &'w World,
    player: Entity,
    holders: PlayerHolders,
    slots: [SlotState; SLOT_COUNT],
}

impl<'w> ClickSlots<'w> {
    pub(crate) fn read(
        world: &'w World,
        player: Entity,
        holders: PlayerHolders,
        holder_slots: &Query<(&Slots, Option<&SlotPolicy>)>,
        stacks: &mut Stacks,
    ) -> Self {
        let mut slots = [SlotState::Missing; SLOT_COUNT];
        for holder in HOLDERS {
            let Ok((content, policy)) = holder_slots.get(holder_entity(&holders, player, holder))
            else {
                continue;
            };
            for index in 0..holder.slot_count() {
                let Some(flat) = flat(MenuSlot::new(holder, index)) else {
                    continue;
                };
                if usize::from(index) >= content.len() {
                    continue;
                }
                let stack = content
                    .get(usize::from(index))
                    .and_then(|entity| planned(entity, flat, &slots, stacks));
                let access = policy.map_or(OPEN, |policy| policy.access(index));
                if let Some(slot) = slots.get_mut(flat) {
                    *slot = SlotState::Holds { stack, access };
                }
            }
        }
        Self {
            world,
            player,
            holders,
            slots,
        }
    }

    pub(crate) fn slot_ref(&self, slot: MenuSlot) -> SlotRef {
        SlotRef {
            holder: holder_entity(&self.holders, self.player, slot.holder),
            index: slot.index,
        }
    }

    pub(crate) fn holds_empty(&self, slot: MenuSlot) -> bool {
        matches!(self.state(slot), SlotState::Holds { stack: None, .. })
    }

    fn state(&self, slot: MenuSlot) -> SlotState {
        flat(slot)
            .and_then(|flat| self.slots.get(flat).copied())
            .unwrap_or(SlotState::Missing)
    }

    fn answer(&self, gate: Gate, slot: MenuSlot, stack: Entity) -> Filter {
        let query = GateQuery {
            slot: self.slot_ref(slot),
            actor: self.player,
            stack,
        };
        gate.answer(self.world, &query)
    }
}

impl mcrs_minecraft_inventory_click::Slots for ClickSlots<'_> {
    type Slot = MenuSlot;
    type Stack = StackHandle;

    fn cursor(&self) -> MenuSlot {
        MenuSlot::new(Holder::Cursor, 0)
    }

    fn inventory(&self, index: u16) -> Option<MenuSlot> {
        (index < Holder::Inventory.slot_count()).then_some(MenuSlot::new(Holder::Inventory, index))
    }

    fn load(&self, slot: MenuSlot) -> Option<Loaded<StackHandle>> {
        let SlotState::Holds { stack, access } = self.state(slot) else {
            return None;
        };
        // As the commit: a gate that asks has no stack to ask about in an empty slot.
        let take = match stack {
            Some(stack) => self.answer(access.take, slot, stack.stack.entity),
            None if access.take == Gate::Refuses => Filter::Refuses,
            None => Filter::Permits,
        };
        Some(Loaded {
            stack,
            limit: access.limit,
            take,
        })
    }

    fn place(&self, slot: MenuSlot, stack: StackHandle) -> Filter {
        match self.state(slot) {
            SlotState::Holds { access, .. } => self.answer(access.place, slot, stack.entity),
            SlotState::Missing => Filter::Refuses,
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
    let max = clamp_stack_size(stacks.get::<MaxStackSize>(entity));
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
            SlotState::Holds {
                stack: Some(stack), ..
            } => Some(stack.stack),
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
        max,
    })
}
