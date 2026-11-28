use bevy_ecs::observer::On;
use bevy_ecs::system::Query;
use bevy_ecs::system::Res;
use mcrs_minecraft_engine_inventory_menu::MenuParts;
use mcrs_minecraft_engine_inventory_menu::Menus;
use mcrs_minecraft_engine_item::RequestResult;
use mcrs_minecraft_engine_item::Selected;
use mcrs_minecraft_engine_item::SlotRef;
use mcrs_minecraft_inventory_client_view::SlotView;
use mcrs_minecraft_inventory_menu::MenuLayout;
use mcrs_minecraft_inventory_player::OFF_HAND;

use crate::MenuSync;
use crate::menu_slots::MenuSlots;
use mcrs_minecraft_vanilla_inventory::RequestPurpose;
use mcrs_minecraft_vanilla_inventory::VanillaInventoryRequests;

pub(crate) fn forget_rejected_swap(
    result: On<RequestResult>,
    players: Query<(&Selected, &Menus)>,
    mut menus: Query<(&MenuLayout, &MenuParts, &mut MenuSync)>,
    requests: Option<Res<VanillaInventoryRequests>>,
) {
    let Some(requests) = requests else {
        return;
    };
    if result.id.source != requests.source
        || result.id.serial != RequestPurpose::HandSwap.serial()
        || result.outcome.is_ok()
    {
        return;
    }
    let player = result.holder;
    let Ok((selected, player_menus)) = players.get(player) else {
        return;
    };
    let Some(open) = player_menus.open() else {
        return;
    };
    let Ok((layout, parts, mut sync)) = menus.get_mut(open) else {
        return;
    };
    let menu = MenuSlots { layout, parts };
    for index in [u16::from(selected.0), OFF_HAND] {
        let slot = SlotRef {
            holder: player,
            index,
        };
        for entry in 0..menu.len() {
            if menu.slot(entry) != Some(slot) {
                continue;
            }
            if let Some(view) = sync.view.entry_mut(usize::from(entry)) {
                *view = SlotView::Unknown;
            }
        }
    }
}
