use crate::DragEntry;
use crate::Menu;
use crate::Slots;
use crate::stays_in_drag;
use crate::view::View;

#[must_use]
pub fn joins_drag<M, S>(menu: &M, slots: &S, index: u16) -> Option<DragEntry>
where
    M: Menu<Slot = S::Slot>,
    S: Slots,
{
    let view = View { menu, slots };
    let carried = view.cursor().ok()?.stack?;
    let target = view.entry(index).ok()?;
    let entry = stays_in_drag::drag_entry(&view, index, &target, carried)?;
    view.place(&target, carried).then_some(entry)
}
