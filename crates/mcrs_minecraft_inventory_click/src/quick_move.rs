use mcrs_minecraft_inventory_plan::Op;

use crate::Click;
use crate::ClickError;
use crate::Clicked;
use crate::Menu;
use crate::Slots;
use crate::pick_up::MouseButton;
use crate::pick_up::drop_cursor;
use crate::view::Ops;
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
    if view.slots.can_stack(moving) {
        for destination in view.menu.quick_move_destinations(index) {
            if left == 0 {
                break;
            }
            let target = view.entry(destination)?;
            let Some(held) = target.stack else {
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
        return Ok(());
    }
    for destination in view.menu.quick_move_destinations(index) {
        let target = view.entry(destination)?;
        if target.stack.is_some() || !view.place(&target, moving) {
            continue;
        }
        let count = left.min(target.cap(moving));
        return ops.push(Op::Split {
            from: source.slot,
            to: target.slot,
            count,
        });
    }
    Ok(())
}
