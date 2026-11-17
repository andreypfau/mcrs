use bevy_ecs::change_detection::DetectChangesMut;
use bevy_ecs::entity::Entity;
use bevy_ecs::query::With;
use bevy_ecs::system::InRef;
use bevy_ecs::system::Local;
use bevy_ecs::system::Query;
use bevy_ecs::system::RegisteredSystemError;
use bevy_ecs::world::World;
use mcrs_minecraft_engine::ability::PlayerGameMode;
use mcrs_minecraft_engine_item::HolderCommandsExt;
use mcrs_minecraft_engine_item::Request;
use mcrs_minecraft_engine_item::Selected;
use mcrs_minecraft_engine_item::SlotPolicy;
use mcrs_minecraft_engine_item::Slots;
use mcrs_minecraft_engine_item::Stacks;
use mcrs_minecraft_game_mode::GameMode;
use mcrs_minecraft_inventory_click::ClickError;
use mcrs_minecraft_inventory_click::ClickPlayer;
use mcrs_minecraft_inventory_click::Drag;
use mcrs_minecraft_inventory_click::Operations;
use mcrs_minecraft_inventory_client_view::ClickSync;
use mcrs_minecraft_inventory_menu::InventoryMenu;
use mcrs_minecraft_inventory_menu::MenuSlot;
use mcrs_minecraft_inventory_player::PlayerMode;
use mcrs_minecraft_inventory_transaction::Op;

use crate::InventoryInput;
use crate::MenuSync;
use crate::click_drag::ClickDrag;
use crate::click_menu::ClickMenu;
use crate::click_slots::ClickSlots;
use crate::click_slots::StackHandle;
use crate::menu_queue::MenuQueue;
use mcrs_minecraft_vanilla_inventory::CLICK_REQUEST;
use mcrs_minecraft_vanilla_inventory::PlayerHolders;

pub(crate) struct ClickPlan {
    drag: Drag,
    request: Option<Request>,
    view: ViewChange,
}

enum ViewChange {
    FullContent,
    Unchanged,
    Claims,
}

pub(crate) fn apply_click(
    world: &mut World,
    input: &InventoryInput,
) -> Result<(), RegisteredSystemError<InRef<'static, InventoryInput>, Option<ClickPlan>>> {
    let Some(plan) = world.run_system_cached_with(plan_click, input)? else {
        return Ok(());
    };
    let &InventoryInput::Click {
        player,
        state_id,
        ref changed,
        ref carried,
        ..
    } = input
    else {
        return Ok(());
    };
    set_drag(world, player, plan.drag);
    if let Some(request) = plan.request {
        world.commands().entity(player).request(request);
    }
    match plan.view {
        ViewChange::FullContent => {
            queue_full_content(world, player);
            return Ok(());
        }
        ViewChange::Unchanged => return Ok(()),
        ViewChange::Claims => {}
    }
    let Some(mut sync) = world.get_mut::<MenuSync>(player) else {
        return Ok(());
    };
    if let ClickSync::FullContent = sync.state_id.sync_after_click(state_id) {
        queue_full_content(world, player);
        return Ok(());
    }
    for claim in changed {
        if let Some(view) = sync.view.entry_mut(usize::from(claim.index)) {
            view.clone_from(&claim.view);
        }
    }
    sync.view.cursor_mut().clone_from(carried);
    Ok(())
}

fn plan_click(
    InRef(input): InRef<InventoryInput>,
    world: &World,
    players: Query<
        (
            &PlayerHolders,
            &Selected,
            Option<&PlayerGameMode>,
            &ClickDrag,
        ),
        (With<Slots>, With<MenuSync>),
    >,
    holders: Query<(&Slots, Option<&SlotPolicy>)>,
    mut stacks: Stacks,
    mut ops: Local<Operations<MenuSlot, StackHandle>>,
) -> Option<ClickPlan> {
    let &InventoryInput::Click {
        player,
        container_id,
        click,
        ..
    } = input
    else {
        return None;
    };
    if container_id != i32::from(InventoryMenu::CONTAINER_ID) {
        return None;
    }
    let (player_holders, selected, game_mode, drag) = players.get(player).ok()?;
    let game_mode = game_mode.map_or(GameMode::Survival, |game_mode| game_mode.0);
    let mode = match game_mode {
        GameMode::Spectator => {
            return Some(ClickPlan {
                drag: drag.0,
                request: None,
                view: ViewChange::FullContent,
            });
        }
        GameMode::Creative => PlayerMode::Creative,
        GameMode::Survival | GameMode::Adventure => PlayerMode::NotCreative,
    };
    let slots = ClickSlots::read(world, player, *player_holders, &holders, &mut stacks);
    let clicker = ClickPlayer {
        mode,
        selected: selected.0,
    };
    let mut drag = drag.0;
    let planned = mcrs_minecraft_inventory_click::request(
        click,
        clicker,
        &ClickMenu(&slots),
        &slots,
        &mut drag,
        &mut ops,
    );
    let request = match planned {
        Err(ClickError::NoSuchEntry) => {
            return Some(ClickPlan {
                drag,
                request: None,
                view: ViewChange::Unchanged,
            });
        }
        Err(ClickError::NoSuchSlot | ClickError::TooManyOps) => None,
        Ok(()) => engine_request(&slots, &ops),
    };
    Some(ClickPlan {
        drag,
        request,
        view: ViewChange::Claims,
    })
}

fn set_drag(world: &mut World, player: Entity, drag: Drag) {
    if let Some(mut current) = world.get_mut::<ClickDrag>(player) {
        current.set_if_neq(ClickDrag(drag));
    }
}

fn queue_full_content(world: &mut World, player: Entity) {
    if let Some(mut queue) = world.get_resource_mut::<MenuQueue>() {
        queue.content.push(player);
    }
}

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
