use mcrs_minecraft_inventory_plan::Op;

use crate::Click;
use crate::ClickError;
use crate::Clicked;
use crate::Menu;
use crate::Slots;
use crate::view::Ops;
use crate::view::View;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Visit {
    First,
    Second,
}

pub(crate) fn pick_up_all<M: Menu<Slot = S::Slot>, S: Slots>(
    view: &View<'_, M, S>,
    click: Click,
    ops: &mut Ops<S>,
) -> Result<(), ClickError> {
    let Clicked::Entry(index) = click.clicked else {
        return Ok(());
    };
    let cursor = view.cursor()?;
    let Some(carried) = cursor.stack else {
        return Ok(());
    };
    let clicked = view.entry(index)?;
    if clicked.stack.is_some() && clicked.takes() {
        return Ok(());
    }
    let entries = view.menu.entry_count();
    let mut count = carried.count;
    for visit in [Visit::First, Visit::Second] {
        for position in 0..entries {
            if count >= carried.max {
                return Ok(());
            }
            let entry = match click.button {
                0 => position,
                _ => entries - 1 - position,
            };
            if !view.menu.permits_pick_up_all(entry) {
                continue;
            }
            let source = view.entry(entry)?;
            let Some(held) = source.stack else {
                continue;
            };
            let left = held.count.saturating_sub(ops.split_from(source.slot));
            if left == 0
                || left > carried.max
                || !view.same(held, carried)
                || !source.takes()
                || (visit == Visit::First && left == held.max)
            {
                continue;
            }
            let taken = left.min(carried.max - count);
            if taken < left && !view.place(&source, held) {
                continue;
            }
            ops.push(Op::Split {
                from: source.slot,
                to: cursor.slot,
                count: taken,
            })?;
            count += taken;
        }
    }
    Ok(())
}
