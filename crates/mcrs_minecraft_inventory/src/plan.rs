use bevy_ecs::entity::Entity;
use mcrs_minecraft_item::slots;
use mcrs_minecraft_protocol::item::ContainerInput;

use crate::menu::PLAYER_MENU_CELLS;
use crate::slot::{MenuSnapshot, Slot, Source, StackView};
use crate::transaction::Op;

pub const SLOT_CLICKED_OUTSIDE: i16 = -999;
const SWAP_OFFHAND_BUTTON: u8 = 40;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Click {
    pub slot: i16,
    pub button: u8,
    pub input: ContainerInput,
    pub creative: bool,
}

/// Records ops against the overlay so every later step sees the earlier
/// ones, exactly as the applier will.
pub struct Planner<'a> {
    pub snapshot: &'a mut MenuSnapshot,
    pub ops: Vec<Op>,
}

impl<'a> Planner<'a> {
    pub fn new(snapshot: &'a mut MenuSnapshot) -> Self {
        Planner {
            snapshot,
            ops: Vec::new(),
        }
    }

    fn player(&self) -> Entity {
        self.snapshot.player
    }

    fn count(&self, source: impl Into<Source>) -> u8 {
        self.snapshot.count(source)
    }

    /// Moves up to `amount` from `from` into `to`, which is empty or holds the
    /// same item; the caller has checked the cell accepts it.
    fn transfer(&mut self, from: Source, to: Slot, amount: u8) -> u8 {
        let Some(source) = self.snapshot.get(from).cloned() else {
            return 0;
        };
        if from == Source::Slot(to) {
            return 0;
        }
        let moving = amount.min(source.count);
        if moving == 0 {
            return 0;
        }
        let target = match self.snapshot.get(to) {
            None => source.with_count(moving),
            Some(target) => target.with_count(target.count + moving),
        };
        self.snapshot.set(to, Some(target));
        let left = source.count - moving;
        self.snapshot
            .set(from, (left > 0).then(|| source.with_count(left)));
        self.ops.push(match from {
            Source::Slot(from) => Op::Transfer {
                from,
                to,
                count: moving,
            },
            Source::Item(item) => Op::Pickup {
                item,
                to,
                count: moving,
            },
        });
        moving
    }

    fn swap(&mut self, a: Slot, b: Slot) {
        let (view_a, view_b) = (self.snapshot.take(a), self.snapshot.take(b));
        self.snapshot.set(a, view_b);
        self.snapshot.set(b, view_a);
        self.ops.push(Op::Swap { a, b });
    }

    fn drop(&mut self, from: Slot, amount: u8) {
        let Some(source) = self.snapshot.get(from).cloned() else {
            return;
        };
        let thrown = amount.min(source.count);
        if thrown == 0 {
            return;
        }
        let left = source.count - thrown;
        self.snapshot
            .set(from, (left > 0).then(|| source.with_count(left)));
        self.ops.push(Op::Drop {
            from,
            count: thrown,
            thrower: self.player(),
        });
    }

    /// Inserts up to `amount` of `from` into the cell, which is empty or holds
    /// the same item; whatever does not fit stays where it was.
    fn safe_insert(&mut self, from: Slot, cell: Slot, amount: u8) {
        let Some(source) = self.snapshot.get(from).cloned() else {
            return;
        };
        let Some(max) = self.snapshot.cell_max(cell, &source) else {
            return;
        };
        let room = max.saturating_sub(self.count(cell));
        self.transfer(Source::Slot(from), cell, amount.min(room));
    }

    /// Merges into same-item cells, then fills the first empty one; `cells`
    /// come in the order they are tried. Returns whether anything moved.
    fn move_to(&mut self, from: Source, cells: &[Slot]) -> bool {
        let merged = self.merge_same(from, cells);
        if self.snapshot.get(from).is_none() {
            return true;
        }
        self.fill_empty(from, cells) || merged
    }

    fn merge_same(&mut self, from: Source, cells: &[Slot]) -> bool {
        let mut moved = false;
        let Some(source) = self.snapshot.get(from).cloned() else {
            return false;
        };
        if !source.stackable {
            return false;
        }
        for &cell in cells {
            let Some(target) = self.snapshot.get(cell) else {
                continue;
            };
            if !target.same(&source) {
                continue;
            }
            let Some(max) = self.snapshot.cell_max(cell, &source) else {
                continue;
            };
            let room = max.saturating_sub(target.count);
            moved |= self.transfer(from, cell, room) > 0;
            if self.snapshot.get(from).is_none() {
                return true;
            }
        }
        moved
    }

    fn fill_empty(&mut self, from: Source, cells: &[Slot]) -> bool {
        let Some(source) = self.snapshot.get(from).cloned() else {
            return false;
        };
        for &cell in cells {
            if self.snapshot.get(cell).is_some() {
                continue;
            }
            let Some(max) = self.snapshot.cell_max(cell, &source) else {
                continue;
            };
            if self.transfer(from, cell, max) > 0 {
                return true;
            }
        }
        false
    }

    /// The cells a picked-up stack merges into, in vanilla's order: the held
    /// slot, the offhand, then the hotbar and main inventory.
    fn pickup_merge_cells(&self) -> Vec<Slot> {
        let player = self.player();
        let held = slots::held(self.snapshot.selected);
        [held, slots::OFFHAND]
            .into_iter()
            .chain(
                slots::HOTBAR
                    .chain(slots::MAIN)
                    .filter(|index| *index != held),
            )
            .map(|index| Slot::new(player, index))
            .collect()
    }

    fn pickup_empty_cells(&self) -> Vec<Slot> {
        let player = self.player();
        slots::HOTBAR
            .chain(slots::MAIN)
            .map(|index| Slot::new(player, index))
            .collect()
    }

    /// How many of the stack the player's inventory can still take.
    pub fn room_for(&self, source: &StackView) -> u32 {
        let mut room = 0u32;
        if source.stackable {
            for cell in self.pickup_merge_cells() {
                if let Some(target) = self.snapshot.get(cell)
                    && target.same(source)
                {
                    room += u32::from(source.max.saturating_sub(target.count));
                }
            }
        }
        for cell in self.pickup_empty_cells() {
            if self.snapshot.get(cell).is_none() {
                room += u32::from(source.max);
            }
        }
        room
    }

    /// Stores a stack the way a pickup does; what does not fit stays in `from`.
    pub fn insert_stack(&mut self, from: Source) -> bool {
        let merged = self.merge_same(from, &self.pickup_merge_cells());
        if self.snapshot.get(from).is_none() {
            return true;
        }
        self.fill_empty(from, &self.pickup_empty_cells()) || merged
    }

    /// Puts a stack back into the player's inventory the way a pickup does,
    /// dropping what does not fit.
    pub fn insert_or_drop(&mut self, from: Slot) {
        self.insert_stack(Source::Slot(from));
        let left = self.count(from);
        self.drop(from, left);
    }

    fn layout_range(&self, range: std::ops::Range<usize>, backwards: bool) -> Vec<Slot> {
        let cells = &self.snapshot.layout[range];
        if backwards {
            cells.iter().rev().copied().collect()
        } else {
            cells.to_vec()
        }
    }

    fn quick_move(&mut self, slot: usize) -> bool {
        let layout_len = self.snapshot.layout.len();
        let from = self.snapshot.layout[slot];
        let Some(stack) = self.snapshot.get(from).cloned() else {
            return false;
        };
        if layout_len != slots::MENU_COUNT {
            let container = layout_len - PLAYER_MENU_CELLS;
            let cells = if slot < container {
                self.layout_range(container..layout_len, true)
            } else {
                self.layout_range(0..container, false)
            };
            return self.move_to(Source::Slot(from), &cells);
        }
        let player = self.player();
        let slot = slot as u16;
        let main_and_hotbar = slots::MAIN.start as usize..slots::HOTBAR.end as usize;
        let cells = if slot == slots::RESULT {
            self.layout_range(main_and_hotbar, true)
        } else if slot < slots::MAIN.start {
            self.layout_range(main_and_hotbar, false)
        } else if let Some(armour) = stack
            .armour
            .filter(|cell| self.snapshot.get(Slot::new(player, *cell)).is_none())
        {
            vec![Slot::new(player, armour)]
        } else if stack.offhand
            && self
                .snapshot
                .get(Slot::new(player, slots::OFFHAND))
                .is_none()
        {
            vec![Slot::new(player, slots::OFFHAND)]
        } else if slots::MAIN.contains(&slot) {
            self.layout_range(
                slots::HOTBAR.start as usize..slots::HOTBAR.end as usize,
                false,
            )
        } else if slots::HOTBAR.contains(&slot) {
            self.layout_range(slots::MAIN.start as usize..slots::MAIN.end as usize, false)
        } else {
            self.layout_range(main_and_hotbar, false)
        };
        self.move_to(Source::Slot(from), &cells)
    }

    /// A click on the open menu, validated against the layout by the caller.
    /// ponytail: drag and double-click are not applied; the full resend rolls
    /// the client's prediction back. Upgrade: the drag header state machine
    /// and the two-pass gather over the layout.
    pub fn click(&mut self, click: Click) {
        let player = self.player();
        let carried_cell = self.snapshot.carried();
        let carried = self.snapshot.get(carried_cell).cloned();
        let primary = click.button == 0;
        let clicked_cell = usize::try_from(click.slot)
            .ok()
            .and_then(|slot| self.snapshot.layout.get(slot).copied());
        match click.input {
            ContainerInput::Pickup | ContainerInput::QuickMove if click.button > 1 => {}
            ContainerInput::Pickup | ContainerInput::QuickMove
                if click.slot == SLOT_CLICKED_OUTSIDE =>
            {
                if let Some(carried) = carried {
                    let amount = if primary { carried.count } else { 1 };
                    self.drop(carried_cell, amount);
                }
            }
            ContainerInput::QuickMove => {
                let Some(slot) = usize::try_from(click.slot).ok() else {
                    return;
                };
                let from = self.snapshot.layout[slot];
                let item = self.snapshot.get(from).map(|stack| stack.key.item);
                while self.quick_move(slot)
                    && self.snapshot.get(from).map(|stack| stack.key.item) == item
                {}
            }
            ContainerInput::Pickup => {
                let Some(cell) = clicked_cell else {
                    return;
                };
                match (self.snapshot.get(cell).cloned(), carried) {
                    (None, None) => {}
                    (None, Some(carried)) => {
                        let amount = if primary { carried.count } else { 1 };
                        self.safe_insert(carried_cell, cell, amount);
                    }
                    (Some(clicked), None) => {
                        let have = clicked.count;
                        let amount = if primary { have } else { have.div_ceil(2) };
                        self.transfer(Source::Slot(cell), carried_cell, amount);
                    }
                    (Some(clicked), Some(carried)) => {
                        let same = clicked.same(&carried);
                        match self.snapshot.cell_max(cell, &carried) {
                            Some(_) if same => {
                                let amount = if primary { carried.count } else { 1 };
                                self.safe_insert(carried_cell, cell, amount);
                            }
                            Some(max) if carried.count <= max => self.swap(cell, carried_cell),
                            Some(_) => {}
                            None if same => {
                                let room = carried.max - carried.count;
                                if room >= clicked.count {
                                    self.transfer(Source::Slot(cell), carried_cell, clicked.count);
                                }
                            }
                            None => {}
                        }
                    }
                }
            }
            ContainerInput::Swap => {
                let (Some(cell), Some(source_cell)) =
                    (clicked_cell, swap_source(player, click.button))
                else {
                    return;
                };
                match (
                    self.snapshot.get(source_cell).cloned(),
                    self.snapshot.get(cell).cloned(),
                ) {
                    (None, None) => {}
                    (None, Some(_)) => {
                        self.transfer(Source::Slot(cell), source_cell, u8::MAX);
                    }
                    (Some(source), None) => {
                        if let Some(max) = self.snapshot.cell_max(cell, &source) {
                            self.transfer(Source::Slot(source_cell), cell, max);
                        }
                    }
                    (Some(source), Some(_)) => {
                        let Some(max) = self.snapshot.cell_max(cell, &source) else {
                            return;
                        };
                        if source.count > max {
                            self.insert_or_drop(cell);
                            self.transfer(Source::Slot(source_cell), cell, max);
                        } else {
                            self.swap(cell, source_cell);
                        }
                    }
                }
            }
            ContainerInput::Clone => {
                let Some(cell) = clicked_cell.filter(|cell| self.snapshot.get(*cell).is_some())
                else {
                    return;
                };
                if !click.creative || carried.is_some() {
                    return;
                }
                let full = self
                    .snapshot
                    .get(cell)
                    .map(|clicked| clicked.with_count(clicked.max));
                self.snapshot.set(carried_cell, full);
                self.ops.push(Op::Clone {
                    from: cell,
                    to: carried_cell,
                });
            }
            ContainerInput::Throw => {
                let Some(cell) = clicked_cell.filter(|cell| self.snapshot.get(*cell).is_some())
                else {
                    return;
                };
                if carried.is_some() {
                    return;
                }
                let amount = if primary { 1 } else { self.count(cell) };
                self.drop(cell, amount);
            }
            ContainerInput::QuickCraft | ContainerInput::PickupAll => {}
        }
    }

    /// The stacks a closing menu hands back: the cursor and the crafting grid.
    pub fn close(&mut self) {
        let player = self.player();
        for index in std::iter::once(slots::CARRIED).chain(slots::CRAFT) {
            let cell = Slot::new(player, index);
            if self.snapshot.get(cell).is_some() {
                self.insert_or_drop(cell);
            }
        }
    }

    pub fn drop_held(&mut self, all: bool) {
        let held = Slot::new(self.player(), slots::held(self.snapshot.selected));
        let amount = if all { self.count(held) } else { 1 };
        self.drop(held, amount);
    }

    pub fn swap_offhand(&mut self) {
        let player = self.player();
        let held = Slot::new(player, slots::held(self.snapshot.selected));
        let offhand = Slot::new(player, slots::OFFHAND);
        if self.snapshot.get(held).is_some() || self.snapshot.get(offhand).is_some() {
            self.swap(held, offhand);
        }
    }
}

fn swap_source(player: Entity, button: u8) -> Option<Slot> {
    match button {
        0..=8 => Some(Slot::new(player, slots::held(button))),
        SWAP_OFFHAND_BUTTON => Some(Slot::new(player, slots::OFFHAND)),
        _ => None,
    }
}
