use crate::Menu;
use crate::Slots;
use crate::view::View;

#[must_use]
pub fn shift_double_click_moves<M, S>(
    menu: &M,
    slots: &S,
    target: u16,
    index: u16,
    alike: impl FnOnce(S::Stack) -> bool,
) -> bool
where
    M: Menu<Slot = S::Slot>,
    S: Slots,
{
    let view = View { menu, slots };
    let (Ok(target), Ok(entry)) = (view.entry(target), view.entry(index)) else {
        return false;
    };
    let Some(held) = entry.stack else {
        return false;
    };
    entry.takes()
        && slots.same_holder(entry.slot, target.slot)
        && held.count <= held.max
        && alike(held.stack)
}
