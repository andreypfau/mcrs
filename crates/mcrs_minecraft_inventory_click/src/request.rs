use mcrs_minecraft_container_click::Click;
use mcrs_minecraft_container_click::ClickKind;
use mcrs_minecraft_container_click::Clicked;

use crate::ClickError;
use crate::ClickPlayer;
use crate::Drag;
use crate::Menu;
use crate::Operations;
use crate::Slots;
use crate::clone;
use crate::drag_click;
use crate::pick_up;
use crate::pick_up_all;
use crate::quick_move;
use crate::swap;
use crate::throw;
use crate::view::View;

pub fn request<M, S>(
    click: Click,
    player: ClickPlayer,
    menu: &M,
    slots: &S,
    drag: &mut Drag,
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
        ClickKind::Drag => drag_click::drag_click(&view, click, player, drag, ops),
        _ if drag.kind().is_some() => {
            drag.reset();
            Ok(())
        }
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
