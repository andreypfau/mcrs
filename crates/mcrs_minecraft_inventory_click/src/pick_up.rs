use mcrs_minecraft_inventory_plan::Op;

use crate::Click;
use crate::ClickError;
use crate::Clicked;
use crate::Menu;
use crate::Slots;
use crate::view::Operation;
use crate::view::Ops;
use crate::view::View;

/// The button of a pick up click and of a quick move click.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MouseButton {
    Primary,
    Secondary,
}

impl MouseButton {
    pub(crate) fn from_button(button: u8) -> Option<Self> {
        match button {
            0 => Some(Self::Primary),
            1 => Some(Self::Secondary),
            _ => None,
        }
    }
}

/// The drop from the cursor of a click outside the window.
pub(crate) fn drop_cursor<M: Menu<Slot = S::Slot>, S: Slots>(
    view: &View<'_, M, S>,
    button: MouseButton,
    ops: &mut Ops<S>,
) -> Result<(), ClickError> {
    let cursor = view.cursor()?;
    let Some(carried) = cursor.stack else {
        return Ok(());
    };
    let count = match button {
        MouseButton::Primary => carried.count,
        MouseButton::Secondary => 1,
    };
    ops.push(Op::Drop {
        from: cursor.slot,
        count,
    })
}

pub(crate) fn pick_up<M: Menu<Slot = S::Slot>, S: Slots>(
    view: &View<'_, M, S>,
    click: Click,
    ops: &mut Ops<S>,
) -> Result<(), ClickError> {
    let Some(button) = MouseButton::from_button(click.button) else {
        return Ok(());
    };
    match click.clicked {
        Clicked::Outside => drop_cursor(view, button, ops),
        Clicked::Nowhere => Ok(()),
        Clicked::Entry(index) => match on_entry(view, index, button)? {
            Some(op) => ops.push(op),
            None => Ok(()),
        },
    }
}

pub(crate) fn on_entry<M: Menu<Slot = S::Slot>, S: Slots>(
    view: &View<'_, M, S>,
    index: u16,
    button: MouseButton,
) -> Result<Option<Operation<S>>, ClickError> {
    let target = view.entry(index)?;
    let cursor = view.cursor()?;
    let (into, from) = (target.slot, cursor.slot);
    let op = match (target.stack, cursor.stack) {
        (None, None) => None,
        (None, Some(carried)) => {
            if !view.place(&target, carried) {
                return Ok(None);
            }
            let count = carried_part(carried.count, button).min(target.cap(carried));
            split(from, into, count)
        }
        (Some(_), _) if !target.takes() => None,
        (Some(held), None) => match button {
            MouseButton::Primary => Some(Op::Swap {
                left: into,
                right: from,
            }),
            MouseButton::Secondary => split(into, from, held.count.div_ceil(2)),
        },
        (Some(held), Some(carried)) if view.place(&target, carried) => {
            if view.same(held, carried) {
                let room = target.cap(carried).saturating_sub(held.count);
                split(from, into, carried_part(carried.count, button).min(room))
            } else if carried.count <= target.cap(carried) {
                Some(Op::Swap {
                    left: into,
                    right: from,
                })
            } else {
                None
            }
        }
        (Some(held), Some(carried)) if view.same(held, carried) => {
            let count = held.count.min(carried.max.saturating_sub(carried.count));
            if count < held.count && !view.place(&target, held) {
                return Ok(None);
            }
            split(into, from, count)
        }
        (Some(_), Some(_)) => None,
    };
    Ok(op)
}

/// `k`: the part of the cursor stack that a place moves.
fn carried_part(count: u8, button: MouseButton) -> u8 {
    match button {
        MouseButton::Primary => count,
        MouseButton::Secondary => 1,
    }
}

fn split<I, T>(from: I, to: I, count: u8) -> Option<Op<I, T>> {
    (count > 0).then_some(Op::Split { from, to, count })
}
