use mcrs_minecraft_container_click::Click;
use mcrs_minecraft_container_click::Clicked;
use mcrs_minecraft_inventory_player::PlayerMode;
use mcrs_minecraft_inventory_transaction::Op;
use mcrs_minecraft_inventory_transaction::Planned;

use crate::ClickError;
use crate::ClickPlayer;
use crate::Menu;
use crate::Slots;
use crate::view::Ops;
use crate::view::View;

pub(crate) fn clone<M: Menu<Slot = S::Slot>, S: Slots>(
    view: &View<'_, M, S>,
    click: Click,
    player: ClickPlayer,
    ops: &mut Ops<S>,
) -> Result<(), ClickError> {
    let Clicked::Entry(index) = click.clicked else {
        return Ok(());
    };
    if player.mode != PlayerMode::Creative {
        return Ok(());
    }
    let cursor = view.cursor()?;
    if cursor.stack.is_some() {
        return Ok(());
    }
    let Some(held) = view.entry(index)?.stack else {
        return Ok(());
    };
    ops.push(Op::Set {
        slot: cursor.slot,
        stack: Some(Planned {
            count: held.max,
            ..held
        }),
        count: held.max,
    })
}
