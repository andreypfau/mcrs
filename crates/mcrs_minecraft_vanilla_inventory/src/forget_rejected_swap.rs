use bevy_ecs::observer::On;
use bevy_ecs::system::Query;
use mcrs_minecraft_engine_item::RequestResult;
use mcrs_minecraft_engine_item::Selected;
use mcrs_minecraft_inventory_view::SlotView;
use mcrs_minecraft_menu::InventoryMenu;
use mcrs_minecraft_menu::MenuSlot;
use mcrs_minecraft_player_slots::Holder;
use mcrs_minecraft_player_slots::OFF_HAND;

use crate::InventoryMenuSync;
use crate::hand_action::HAND_SWAP_REQUEST;

/// Makes the client view of the hand and of the off hand not known when their swap has a
/// rejection, because the client showed the swap before the answer came (INV-SEL-006).
pub(crate) fn forget_rejected_swap(
    result: On<RequestResult>,
    mut players: Query<(&Selected, &mut InventoryMenuSync)>,
) {
    if result.id != HAND_SWAP_REQUEST || result.outcome.is_ok() {
        return;
    }
    let Ok((selected, mut sync)) = players.get_mut(result.holder) else {
        return;
    };
    for index in [u16::from(selected.0), OFF_HAND] {
        let Some(entry) = InventoryMenu::index(MenuSlot::new(Holder::Inventory, index)) else {
            continue;
        };
        if let Some(view) = sync.view.entry_mut(usize::from(entry)) {
            *view = SlotView::Unknown;
        }
    }
}
