use mcrs_minecraft_inventory_plan::Op;
use mcrs_minecraft_inventory_plan::Planned;
use mcrs_minecraft_player_slots::Slot;
use mcrs_minecraft_player_slots::Stacking;
use mcrs_minecraft_player_slots::insert;
use mcrs_minecraft_player_slots::swap_button_slot;

use crate::Click;
use crate::ClickError;
use crate::ClickPlayer;
use crate::Clicked;
use crate::Menu;
use crate::Slots;
use crate::view::Ops;
use crate::view::Seat;
use crate::view::View;

pub(crate) fn swap<M: Menu<Slot = S::Slot>, S: Slots>(
    view: &View<'_, M, S>,
    click: Click,
    player: ClickPlayer,
    ops: &mut Ops<S>,
) -> Result<(), ClickError> {
    let Clicked::Entry(index) = click.clicked else {
        return Ok(());
    };
    let Some(key) = swap_button_slot(click.button) else {
        return Ok(());
    };
    let hotbar = view.slots.inventory(key).ok_or(ClickError::NoSuchSlot)?;
    let hotbar = view.load(hotbar)?;
    let target = view.entry(index)?;
    // A swap of a slot with itself changes nothing, and the planner rejects it as `same_slot`.
    if target.slot == hotbar.slot {
        return Ok(());
    }
    let exchange = |left: &Seat<S>, right: &Seat<S>| Op::Swap {
        left: left.slot,
        right: right.slot,
    };
    match (target.stack, hotbar.stack) {
        (None, None) => Ok(()),
        (Some(_), None) if target.takes() => ops.push(exchange(&target, &hotbar)),
        (Some(_), None) => Ok(()),
        (None, Some(keyed)) if !view.place(&target, keyed) => Ok(()),
        (None, Some(keyed)) if keyed.count <= target.cap(keyed) => {
            ops.push(exchange(&hotbar, &target))
        }
        (None, Some(keyed)) => fill::<S>(&target, &hotbar, keyed, ops),
        (Some(_), Some(keyed)) if !target.takes() || !view.place(&target, keyed) => Ok(()),
        (Some(_), Some(keyed)) if keyed.count <= target.cap(keyed) => {
            ops.push(exchange(&target, &hotbar))
        }
        (Some(held), Some(keyed)) => {
            insert_held(view, &target, held, player.selected, ops)?;
            fill::<S>(&target, &hotbar, keyed, ops)
        }
    }
}

fn fill<S: Slots>(
    target: &Seat<S>,
    hotbar: &Seat<S>,
    keyed: Planned<S::Stack>,
    ops: &mut Ops<S>,
) -> Result<(), ClickError> {
    ops.push(Op::Split {
        from: hotbar.slot,
        to: target.slot,
        count: target.cap(keyed),
    })
}

/// Moves the stack of `target` into the player inventory by the insert rule, and drops the part
/// that finds no room.
fn insert_held<M: Menu<Slot = S::Slot>, S: Slots>(
    view: &View<'_, M, S>,
    target: &Seat<S>,
    held: Planned<S::Stack>,
    selected: u8,
    ops: &mut Ops<S>,
) -> Result<(), ClickError> {
    let mut pushed: Result<(), ClickError> = Ok(());
    let slot = |index: u16| {
        let Some(slot) = view.slots.inventory(index) else {
            return Slot::Other;
        };
        // The source cannot receive its own items.
        if slot == target.slot {
            return Slot::Other;
        }
        let Ok(seen) = view.load(slot) else {
            return Slot::Other;
        };
        match seen.stack {
            None => Slot::Empty,
            Some(stack) if view.same(stack, held) && view.slots.can_stack(stack) => {
                match seen.cap(stack).checked_sub(stack.count) {
                    Some(room) if room > 0 => Slot::Alike { room },
                    _ => Slot::Other,
                }
            }
            Some(_) => Slot::Other,
        }
    };
    let merge = |index: u16, count: u8| {
        if pushed.is_err() {
            return;
        }
        pushed = match view.slots.inventory(index) {
            Some(to) => ops.push(Op::Split {
                from: target.slot,
                to,
                count,
            }),
            None => Err(ClickError::NoSuchSlot),
        };
    };
    let stacking = if view.slots.can_stack(held) {
        Stacking::Stacks
    } else {
        Stacking::Alone
    };
    let insertion = insert(held.count, held.max, stacking, selected, slot, merge);
    pushed?;
    if let Some(empty) = insertion.empty
        && insertion.filled > 0
    {
        let to = view.slots.inventory(empty).ok_or(ClickError::NoSuchSlot)?;
        ops.push(Op::Split {
            from: target.slot,
            to,
            count: insertion.filled,
        })?;
    }
    if insertion.rest > 0 {
        ops.push(Op::Drop {
            from: target.slot,
            count: insertion.rest,
        })?;
    }
    Ok(())
}
