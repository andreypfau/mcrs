use mcrs_minecraft_inventory_plan::Op;

use crate::Click;
use crate::ClickError;
use crate::Clicked;
use crate::Menu;
use crate::Slots;
use crate::view::Ops;
use crate::view::View;

/// The player can always drop items on the server, so the throw reads no such condition.
pub(crate) fn throw<M: Menu<Slot = S::Slot>, S: Slots>(
    view: &View<'_, M, S>,
    click: Click,
    ops: &mut Ops<S>,
) -> Result<(), ClickError> {
    let Clicked::Entry(index) = click.clicked else {
        return Ok(());
    };
    if view.cursor()?.stack.is_some() {
        return Ok(());
    }
    let source = view.entry(index)?;
    let Some(held) = source.stack else {
        return Ok(());
    };
    if !source.takes() {
        return Ok(());
    }
    let count = match click.button {
        0 => 1,
        _ => held.count,
    };
    ops.push(Op::Drop {
        from: source.slot,
        count,
    })
}
