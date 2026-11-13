use bevy_ecs::entity::Entity;
use bevy_ecs::message::MessageReader;
use bevy_ecs::system::Commands;
use bevy_ecs::system::Local;
use bevy_ecs::system::Query;
use bevy_ecs::system::ResMut;
use mcrs_minecraft_engine::ability::PlayerGameMode;
use mcrs_minecraft_engine_item::HolderCommandsExt;
use mcrs_minecraft_engine_item::Request;
use mcrs_minecraft_engine_item::RequestId;
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

use crate::InventoryClick;
use crate::InventoryMenuSync;
use crate::PlayerHolders;
use crate::click_backlog::ClickBacklog;
use crate::click_drag::ClickDrag;
use crate::click_menu::ClickMenu;
use crate::click_slots::ClickSlots;
use crate::click_slots::StackHandle;
use crate::menu_queue::MenuQueue;

const CLICK_REQUEST: RequestId = RequestId(0);

#[derive(Default)]
pub(crate) struct ClickScratch {
    clicks: Vec<InventoryClick>,
    requested: Vec<Entity>,
    ops: Operations<MenuSlot, StackHandle>,
}

/// Makes the request of each click in the inventory menu and records what the client claims to
/// hold after it (ITX-JAV-001, ITX-JAV-002, ITX-JAV-004). A click of a player whose request of
/// this run has not applied yet waits for the next run, so that each request reads the slots
/// that the one before it left (ITX-JAV-003).
#[expect(
    clippy::too_many_arguments,
    reason = "a click reads the player, its holders and their stacks, and writes the view"
)]
pub(crate) fn apply_clicks(
    mut clicks: MessageReader<InventoryClick>,
    mut backlog: ResMut<ClickBacklog>,
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
    mut scratch: Local<ClickScratch>,
) {
    let scratch = &mut *scratch;
    scratch.requested.clear();
    scratch.clicks.append(&mut backlog.0);
    scratch.clicks.extend(clicks.read().cloned());
    for click in scratch.clicks.drain(..) {
        if scratch.requested.contains(&click.player) {
            backlog.0.push(click);
            continue;
        }
        if click.container_id != i32::from(InventoryMenu::CONTAINER_ID) {
            continue;
        }
        let Ok((inventory, player_holders, selected, game_mode, mut sync, mut drag)) =
            players.get_mut(click.player)
        else {
            continue;
        };
        let game_mode = game_mode.map_or(GameMode::Survival, |game_mode| game_mode.0);
        let mode = match game_mode {
            GameMode::Spectator => {
                queue.content.push(click.player);
                continue;
            }
            GameMode::Creative => PlayerMode::Creative,
            GameMode::Survival | GameMode::Adventure => PlayerMode::NotCreative,
        };
        let slots = ClickSlots::read(
            click.player,
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
            click.click,
            clicker,
            &ClickMenu(&slots),
            &slots,
            &mut drag.0,
            &mut scratch.ops,
        );
        match planned {
            Err(ClickError::NoSuchEntry) => continue,
            Err(ClickError::NoSuchSlot | ClickError::TooManyOps) => {}
            Ok(()) => {
                if let Some(request) = engine_request(&slots, &scratch.ops) {
                    commands.entity(click.player).request(request);
                    scratch.requested.push(click.player);
                }
            }
        }
        match sync.state_id.sync_after_click(click.state_id) {
            ClickSync::FullContent => queue.content.push(click.player),
            ClickSync::CompareEntries => {
                for claim in click.changed {
                    if let Some(view) = sync.view.entry_mut(usize::from(claim.index)) {
                        *view = claim.view;
                    }
                }
                *sync.view.cursor_mut() = click.carried;
                queue.compare.push(click.player);
            }
        }
    }
}

/// The request of `ops`, or `None` when it changes nothing or holds a `drop`.
// chisle: the engine request has no `drop`, so a throw and a drop from the cursor apply nothing
// and the comparison corrects the client; a `drop` of the engine request that spawns the item
// entity of IEN-SPN-001 lifts it.
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
            Op::Drop { .. } => return None,
        };
    }
    Some(request)
}
