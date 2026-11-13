use crate::Click;
use crate::ClickError;
use crate::ClickKind;
use crate::ClickPlayer;
use crate::Clicked;
use crate::Menu;
use crate::Operations;
use crate::Slots;
use crate::clone;
use crate::pick_up;
use crate::pick_up_all;
use crate::quick_move;
use crate::swap;
use crate::throw;
use crate::view::View;

/// Writes into `ops` the operations of the request that `click` of `player` gives in `menu`.
///
/// `ops` holds an empty list when the conditions of the click are false, and nothing after an
/// error. The operations apply by construction when `slots` does not change before they are
/// planned; the clone click gives a `set` that makes items.
pub fn request<M, S>(
    click: Click,
    player: ClickPlayer,
    menu: &M,
    slots: &S,
    ops: &mut Operations<S::Slot, S::Stack>,
) -> Result<(), ClickError>
where
    M: Menu<Slot = S::Slot>,
    S: Slots,
{
    ops.clear();
    if let Clicked::Entry(index) = click.clicked
        && index >= menu.entry_count()
    {
        return Err(ClickError::NoSuchEntry);
    }
    let view = View { menu, slots };
    let result = match click.kind {
        ClickKind::PickUp => pick_up::pick_up(&view, click, ops),
        ClickKind::QuickMove => quick_move::quick_move(&view, click, ops),
        ClickKind::Swap => swap::swap(&view, click, player, ops),
        ClickKind::Clone => clone::clone(&view, click, player, ops),
        ClickKind::Throw => throw::throw(&view, click, ops),
        ClickKind::PickUpAll => pick_up_all::pick_up_all(&view, click, ops),
    };
    if result.is_err() {
        ops.clear();
    }
    result
}
