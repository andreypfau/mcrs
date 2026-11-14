use bevy_ecs::system::Commands;
use bevy_ecs::system::InRef;
use bevy_ecs::system::Local;
use bevy_ecs::system::Query;
use bevy_ecs::system::ResMut;
use mcrs_minecraft_engine::ability::PlayerGameMode;
use mcrs_minecraft_engine_item::HolderCommandsExt;
use mcrs_minecraft_engine_item::Request;
use mcrs_minecraft_engine_item::Selected;
use mcrs_minecraft_engine_item::Slots;
use mcrs_minecraft_engine_item::Stacks;
use mcrs_minecraft_game_mode::GameMode;
use mcrs_minecraft_inventory_click::ClickError;
use mcrs_minecraft_inventory_click::ClickPlayer;
use mcrs_minecraft_inventory_click::Operations;
use mcrs_minecraft_inventory_plan::Op;
use mcrs_minecraft_inventory_view::ClickSync;
use mcrs_minecraft_menu::InventoryMenu;
use mcrs_minecraft_menu::MenuSlot;
use mcrs_minecraft_player_slots::PlayerMode;

use crate::InventoryInput;
use crate::InventoryMenuSync;
use crate::PlayerHolders;
use crate::click_drag::ClickDrag;
use crate::click_menu::ClickMenu;
use crate::click_slots::ClickSlots;
use crate::click_slots::StackHandle;
use crate::menu_queue::MenuQueue;
use crate::request_ids::CLICK_REQUEST;

/// Makes the request of a click in the inventory menu and records what the client claims to hold
/// after it (ITX-JAV-001, ITX-JAV-002, ITX-JAV-004).
pub(crate) fn apply_click(
    InRef(input): InRef<InventoryInput>,
    mut queue: ResMut<MenuQueue>,
    mut players: Query<(
        &Slots,
        &PlayerHolders,
        &Selected,
        Option<&PlayerGameMode>,
        &mut InventoryMenuSync,
        &mut ClickDrag,
    )>,
    holders: Query<&Slots>,
    mut stacks: Stacks,
    mut commands: Commands,
    mut ops: Local<Operations<MenuSlot, StackHandle>>,
) {
    let &InventoryInput::Click {
        player,
        container_id,
        state_id,
        click,
        ref changed,
        ref carried,
    } = input
    else {
        return;
    };
    if container_id != i32::from(InventoryMenu::CONTAINER_ID) {
        return;
    }
    let Ok((inventory, player_holders, selected, game_mode, mut sync, mut drag)) =
        players.get_mut(player)
    else {
        return;
    };
    let game_mode = game_mode.map_or(GameMode::Survival, |game_mode| game_mode.0);
    let mode = match game_mode {
        GameMode::Spectator => {
            queue.content.push(player);
            return;
        }
        GameMode::Creative => PlayerMode::Creative,
        GameMode::Survival | GameMode::Adventure => PlayerMode::NotCreative,
    };
    let slots = ClickSlots::read(
        player,
        inventory,
        *player_holders,
        &holders,
        mode,
        &mut stacks,
    );
    let clicker = ClickPlayer {
        mode,
        selected: selected.0,
    };
    let planned = mcrs_minecraft_inventory_click::request(
        click,
        clicker,
        &ClickMenu(&slots),
        &slots,
        &mut drag.0,
        &mut ops,
    );
    match planned {
        Err(ClickError::NoSuchEntry) => return,
        Err(ClickError::NoSuchSlot | ClickError::TooManyOps) => {}
        Ok(()) => {
            if let Some(request) = engine_request(&slots, &ops) {
                commands.entity(player).request(request);
            }
        }
    }
    match sync.state_id.sync_after_click(state_id) {
        ClickSync::FullContent => queue.content.push(player),
        ClickSync::CompareEntries => {
            for claim in changed {
                if let Some(view) = sync.view.entry_mut(usize::from(claim.index)) {
                    view.clone_from(&claim.view);
                }
            }
            sync.view.cursor_mut().clone_from(carried);
            queue.compare.push(player);
        }
    }
}

/// The request of `ops`, or `None` when it changes nothing.
fn engine_request(slots: &ClickSlots, ops: &Operations<MenuSlot, StackHandle>) -> Option<Request> {
    if ops.is_empty() {
        return None;
    }
    let mut request = Request::new(CLICK_REQUEST);
    for op in ops.iter() {
        request = match op {
            Op::Split { from, to, count } => {
                request.split(slots.slot_ref(from), slots.slot_ref(to), count)
            }
            Op::Merge { from, to } => request.merge(slots.slot_ref(from), slots.slot_ref(to)),
            Op::Swap { left, right } => request.swap(slots.slot_ref(left), slots.slot_ref(right)),
            Op::Set { slot, stack, count } => request.set(
                slots.slot_ref(slot),
                stack.map(|stack| stack.stack.entity),
                count,
            ),
            Op::Drop { from, count } => request.drop(slots.slot_ref(from), count),
        };
    }
    Some(request)
}
