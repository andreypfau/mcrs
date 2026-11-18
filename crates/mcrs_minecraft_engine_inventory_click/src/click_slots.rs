use core::iter;

use bevy_ecs::entity::Entity;
use bevy_ecs::system::Query;
use bevy_ecs::world::World;
use mcrs_minecraft_engine_inventory_menu::MenuParts;
use mcrs_minecraft_engine_item::Gate;
use mcrs_minecraft_engine_item::GateQuery;
use mcrs_minecraft_engine_item::SlotAccess;
use mcrs_minecraft_engine_item::SlotPolicy;
use mcrs_minecraft_engine_item::SlotRef;
use mcrs_minecraft_engine_item::Slots;
use mcrs_minecraft_engine_item::Stacks;
use mcrs_minecraft_engine_item::stack_capacity;
use mcrs_minecraft_inventory_menu::MenuLayout;
use mcrs_minecraft_inventory_menu::worn_slot;
use mcrs_minecraft_inventory_player::EquipSlot;
use mcrs_minecraft_inventory_player::Holder;
use mcrs_minecraft_inventory_player::Stacking;
use mcrs_minecraft_inventory_transaction::Filter;
use mcrs_minecraft_inventory_transaction::Loaded;
use mcrs_minecraft_inventory_transaction::Planned;
use mcrs_minecraft_item_stack::Damage;
use mcrs_minecraft_item_stack::Equippable;
use mcrs_minecraft_item_stack::MaxStackSize;

const CAPACITY: usize = MenuLayout::MAX_LEN as usize + 42;

const OPEN: SlotAccess = SlotAccess {
    limit: 99,
    take: Gate::Permits,
    place: Gate::Permits,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct StackHandle {
    pub(crate) entity: Entity,
    alike: usize,
    stacking: Stacking,
    worn: Option<EquipSlot>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SlotState {
    Missing,
    Holds {
        stack: Option<Planned<StackHandle>>,
        access: SlotAccess,
    },
}

#[derive(Clone, Copy, Debug)]
struct KnownSlot {
    slot: SlotRef,
    state: SlotState,
}

const UNKNOWN: KnownSlot = KnownSlot {
    slot: SlotRef {
        holder: Entity::PLACEHOLDER,
        index: 0,
    },
    state: SlotState::Missing,
};

pub(crate) struct ClickSlots<'w> {
    world: &'w World,
    player: Entity,
    cursor: SlotRef,
    slots: [KnownSlot; CAPACITY],
    len: usize,
}

impl<'w> ClickSlots<'w> {
    pub(crate) fn read(
        world: &'w World,
        player: Entity,
        layout: &MenuLayout,
        parts: &MenuParts,
        cursor: SlotRef,
        holder_slots: &Query<(&Slots, Option<&SlotPolicy>)>,
        stacks: &mut Stacks,
    ) -> Self {
        let mut read = Self {
            world,
            player,
            cursor,
            slots: [UNKNOWN; CAPACITY],
            len: 0,
        };
        let entries = (0..layout.len()).filter_map(|index| parts.slot(layout.entry(index)?));
        let inventory = (0..Holder::Inventory.slot_count()).map(|index| SlotRef {
            holder: player,
            index,
        });
        for slot in entries.chain(iter::once(cursor)).chain(inventory) {
            read.add(slot, holder_slots, stacks);
        }
        read
    }

    fn add(
        &mut self,
        slot: SlotRef,
        holder_slots: &Query<(&Slots, Option<&SlotPolicy>)>,
        stacks: &mut Stacks,
    ) {
        if self.find(slot).is_some() {
            return;
        }
        let Ok((content, policy)) = holder_slots.get(slot.holder) else {
            return;
        };
        let index = usize::from(slot.index);
        if index >= content.len() {
            return;
        }
        let known = self.slots.get(..self.len).unwrap_or_default();
        let stack = content
            .get(index)
            .and_then(|entity| planned(entity, self.len, known, stacks));
        let access = policy.map_or(OPEN, |policy| policy.access(slot.index));
        let Some(free) = self.slots.get_mut(self.len) else {
            return;
        };
        *free = KnownSlot {
            slot,
            state: SlotState::Holds { stack, access },
        };
        self.len += 1;
    }

    fn find(&self, slot: SlotRef) -> Option<&KnownSlot> {
        let known = self.slots.get(..self.len).unwrap_or_default();
        known.iter().find(|known| known.slot == slot)
    }

    fn state(&self, slot: SlotRef) -> SlotState {
        self.find(slot)
            .map_or(SlotState::Missing, |known| known.state)
    }

    pub(crate) fn worn(&self, slot: SlotRef) -> Option<EquipSlot> {
        match self.state(slot) {
            SlotState::Holds {
                stack: Some(stack), ..
            } => stack.stack.worn,
            SlotState::Holds { stack: None, .. } | SlotState::Missing => None,
        }
    }

    fn answer(&self, gate: Gate, slot: SlotRef, stack: Entity) -> Filter {
        let query = GateQuery {
            slot,
            actor: self.player,
            stack,
        };
        gate.answer(self.world, &query)
    }
}

impl mcrs_minecraft_inventory_click::Slots for ClickSlots<'_> {
    type Slot = SlotRef;
    type Stack = StackHandle;

    fn cursor(&self) -> SlotRef {
        self.cursor
    }

    fn inventory(&self, index: u16) -> Option<SlotRef> {
        (index < Holder::Inventory.slot_count()).then_some(SlotRef {
            holder: self.player,
            index,
        })
    }

    fn load(&self, slot: SlotRef) -> Option<Loaded<StackHandle>> {
        let SlotState::Holds { stack, access } = self.state(slot) else {
            return None;
        };
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

    fn place(&self, slot: SlotRef, stack: StackHandle) -> Filter {
        match self.state(slot) {
            SlotState::Holds { access, .. } => self.answer(access.place, slot, stack.entity),
            SlotState::Missing => Filter::Refuses,
        }
    }

    fn same_holder(&self, left: SlotRef, right: SlotRef) -> bool {
        left.holder == right.holder
    }

    fn equal(&self, left: StackHandle, right: StackHandle) -> bool {
        left.alike == right.alike
    }

    fn can_stack(&self, stack: Planned<StackHandle>) -> bool {
        stack.max > 1 && stack.stack.stacking == Stacking::Stacks
    }
}

fn planned(
    entity: Entity,
    position: usize,
    known: &[KnownSlot],
    stacks: &mut Stacks,
) -> Option<Planned<StackHandle>> {
    let kind = stacks.kind(entity)?;
    let count = stacks.count(entity)?;
    let max = stack_capacity(stacks.get::<MaxStackSize>(entity));
    let stacking = if stacks
        .get::<Damage>(entity)
        .is_some_and(|damage| damage.0 > 0)
    {
        Stacking::Alone
    } else {
        Stacking::Stacks
    };
    let worn = (stacks.get::<Equippable>(entity)).and_then(|equippable| worn_slot(equippable.slot));
    let alike = (known.iter())
        .filter_map(|known| match known.state {
            SlotState::Holds {
                stack: Some(stack), ..
            } => Some(stack.stack),
            _ => None,
        })
        .find(|other| stacks.mergeable(other.entity, entity))
        .map_or(position, |other| other.alike);
    Some(Planned {
        stack: StackHandle {
            entity,
            alike,
            stacking,
            worn,
        },
        kind: kind.0,
        count: count.get(),
        max,
    })
}
