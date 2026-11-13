use mcrs_minecraft_inventory_plan::Filter;
use mcrs_minecraft_inventory_plan::Op;
use mcrs_minecraft_inventory_plan::Planned;

use crate::ClickError;
use crate::Menu;
use crate::Operations;
use crate::Slots;
use crate::seen::Seen;

/// The menu and the slots of one click, with the notation of the specification.
pub(crate) struct View<'a, M, S> {
    pub(crate) menu: &'a M,
    pub(crate) slots: &'a S,
}

pub(crate) type Seat<S> = Seen<<S as Slots>::Slot, <S as Slots>::Stack>;
pub(crate) type Ops<S> = Operations<<S as Slots>::Slot, <S as Slots>::Stack>;
pub(crate) type Operation<S> = Op<<S as Slots>::Slot, Planned<<S as Slots>::Stack>>;

impl<M: Menu<Slot = S::Slot>, S: Slots> View<'_, M, S> {
    pub(crate) fn load(&self, slot: S::Slot) -> Result<Seat<S>, ClickError> {
        let loaded = self.slots.load(slot).ok_or(ClickError::NoSuchSlot)?;
        Ok(Seen {
            slot,
            stack: loaded.stack.filter(|stack| stack.count > 0),
            limit: loaded.limit,
            take: loaded.take,
        })
    }

    pub(crate) fn entry(&self, index: u16) -> Result<Seat<S>, ClickError> {
        let slot = self.menu.slot(index).ok_or(ClickError::NoSuchEntry)?;
        self.load(slot).map_err(|_| ClickError::NoSuchEntry)
    }

    pub(crate) fn cursor(&self) -> Result<Seat<S>, ClickError> {
        self.load(self.slots.cursor())
    }

    /// `same(X, Y)` for the stacks of two slots that are not empty.
    pub(crate) fn same(&self, left: Planned<S::Stack>, right: Planned<S::Stack>) -> bool {
        left.kind == right.kind && self.slots.equal(left.stack, right.stack)
    }

    pub(crate) fn place(&self, slot: &Seat<S>, stack: Planned<S::Stack>) -> bool {
        self.slots.place(slot.slot, stack.stack) == Filter::Permits
    }
}
