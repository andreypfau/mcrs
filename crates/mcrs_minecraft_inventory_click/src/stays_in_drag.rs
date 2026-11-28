use mcrs_minecraft_inventory_transaction::Planned;

use crate::DragEntry;
use crate::Menu;
use crate::Slots;
use crate::drag_click;
use crate::view::Seat;
use crate::view::View;

#[must_use]
pub fn stays_in_drag<M, S>(menu: &M, slots: &S, index: u16) -> Option<DragEntry>
where
    M: Menu<Slot = S::Slot>,
    S: Slots,
{
    let view = View { menu, slots };
    let carried = view.cursor().ok()?.stack?;
    let target = view.entry(index).ok()?;
    drag_entry(&view, index, &target, carried)
}

pub(crate) fn drag_entry<M, S>(
    view: &View<'_, M, S>,
    index: u16,
    target: &Seat<S>,
    carried: Planned<S::Stack>,
) -> Option<DragEntry>
where
    M: Menu<Slot = S::Slot>,
    S: Slots,
{
    if !drag_click::holds(view, target, carried) || !view.menu.permits_drag(index) {
        return None;
    }
    Some(DragEntry {
        held: target.stack.map_or(0, |held| held.count),
        cap: target.cap(carried),
    })
}
