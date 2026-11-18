use mcrs_minecraft_container_click::Click;
use mcrs_minecraft_container_click::Clicked;
use mcrs_minecraft_inventory_transaction::Op;
use mcrs_minecraft_inventory_transaction::Planned;

use crate::ClickError;
use crate::Menu;
use crate::Operations;
use crate::Slots;
use crate::pick_up::MouseButton;
use crate::pick_up::drop_cursor;
use crate::seen::Seen;
use crate::view::Ops;
use crate::view::Seat;
use crate::view::View;

pub(crate) fn quick_move<M: Menu<Slot = S::Slot>, S: Slots>(
    view: &View<'_, M, S>,
    click: Click,
    ops: &mut Ops<S>,
) -> Result<(), ClickError> {
    let Some(button) = MouseButton::from_button(click.button) else {
        return Ok(());
    };
    let index = match click.clicked {
        Clicked::Outside => return drop_cursor(view, button, ops),
        Clicked::Nowhere => return Ok(()),
        Clicked::Entry(index) => index,
    };
    let source = view.entry(index)?;
    let Some(moving) = source.stack else {
        return Ok(());
    };
    if !source.takes() {
        return Ok(());
    }
    let mut left = moving.count;
    while left > 0 {
        let start = ops.clone();
        let remaining = Planned {
            count: left,
            ..moving
        };
        let moved = pass(view, index, &source, remaining, &start, ops)?;
        if moved == 0 {
            break;
        }
        left = left.saturating_sub(moved);
    }
    Ok(())
}

fn pass<M: Menu<Slot = S::Slot>, S: Slots>(
    view: &View<'_, M, S>,
    index: u16,
    source: &Seat<S>,
    moving: Planned<S::Stack>,
    start: &Ops<S>,
    ops: &mut Ops<S>,
) -> Result<u8, ClickError> {
    let holds_empty = |entry: u16| {
        let seat = view.entry(entry).ok();
        seat.is_some_and(|seat| held_at_start(&seat, moving, start).is_none())
    };
    let mut left = moving.count;
    if view.slots.can_stack(moving) {
        for destination in view.menu.quick_move_destinations(index, &holds_empty) {
            if left == 0 {
                break;
            }
            let target = view.entry(destination)?;
            let Some(held) = held_at_start(&target, moving, start) else {
                continue;
            };
            if !view.same(moving, held) || !view.place(&target, moving) {
                continue;
            }
            let count = left.min(target.cap(held).saturating_sub(held.count));
            if count > 0 {
                ops.push(Op::Split {
                    from: source.slot,
                    to: target.slot,
                    count,
                })?;
                left -= count;
            }
        }
    }
    if left == 0 {
        return Ok(moving.count);
    }
    for destination in view.menu.quick_move_destinations(index, &holds_empty) {
        let target = view.entry(destination)?;
        if held_at_start(&target, moving, start).is_some() || !view.place(&target, moving) {
            continue;
        }
        let count = left.min(target.cap(moving));
        ops.push(Op::Split {
            from: source.slot,
            to: target.slot,
            count,
        })?;
        left -= count;
        break;
    }
    Ok(moving.count.saturating_sub(left))
}

fn held_at_start<I: Copy + PartialEq, T: Copy>(
    seat: &Seen<I, T>,
    moving: Planned<T>,
    start: &Operations<I, T>,
) -> Option<Planned<T>> {
    let given = start.split_to(seat.slot);
    match seat.stack {
        Some(stack) => Some(Planned {
            count: stack.count.saturating_add(given),
            ..stack
        }),
        None if given > 0 => Some(Planned {
            count: given,
            ..moving
        }),
        None => None,
    }
}
