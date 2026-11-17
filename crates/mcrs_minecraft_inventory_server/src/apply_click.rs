use bevy_ecs::change_detection::DetectChangesMut;
use bevy_ecs::entity::Entity;
use bevy_ecs::query::With;
use bevy_ecs::system::InRef;
use bevy_ecs::system::Local;
use bevy_ecs::system::Query;
use bevy_ecs::system::RegisteredSystemError;
use bevy_ecs::world::World;
use mcrs_minecraft_engine::ability::PlayerGameMode;
use mcrs_minecraft_engine_inventory_menu::EntryPressed;
use mcrs_minecraft_engine_inventory_menu::MenuButtons;
use mcrs_minecraft_engine_inventory_menu::MenuParts;
use mcrs_minecraft_engine_inventory_menu::Menus;
use mcrs_minecraft_engine_item::HolderCommandsExt;
use mcrs_minecraft_engine_item::Request;
use mcrs_minecraft_engine_item::Selected;
use mcrs_minecraft_engine_item::SlotPolicy;
use mcrs_minecraft_engine_item::SlotRef;
use mcrs_minecraft_engine_item::Slots;
use mcrs_minecraft_engine_item::Stacks;
use mcrs_minecraft_game_mode::GameMode;
use mcrs_minecraft_inventory_click::ClickError;
use mcrs_minecraft_inventory_click::ClickKind;
use mcrs_minecraft_inventory_click::ClickPlayer;
use mcrs_minecraft_inventory_click::Clicked;
use mcrs_minecraft_inventory_click::Drag;
use mcrs_minecraft_inventory_click::Operations;
use mcrs_minecraft_inventory_client_view::ClickSync;
use mcrs_minecraft_inventory_menu::MenuLayout;
use mcrs_minecraft_inventory_player::PlayerMode;
use mcrs_minecraft_inventory_transaction::Op;

use crate::InventoryInput;
use crate::MenuSync;
use crate::click_drag::ClickDrag;
use crate::click_menu::ClickMenu;
use crate::click_slots::ClickSlots;
use crate::click_slots::StackHandle;
use crate::cursor_slot::cursor_slot;
use crate::menu_queue::MenuQueue;
use crate::menu_slots::MenuSlots;
use mcrs_minecraft_vanilla_inventory::CLICK_REQUEST;
use mcrs_minecraft_vanilla_inventory::PlayerHolders;

pub(crate) struct ClickPlan {
    menu: Entity,
    drag: Drag,
    request: Option<Request>,
    pressed: Option<EntryPressed>,
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
    if let Some(pressed) = plan.pressed {
        world.write_message(pressed);
    }
    match plan.view {
        ViewChange::FullContent => {
            queue_full_content(world, plan.menu);
            return Ok(());
        }
        ViewChange::Unchanged => return Ok(()),
        ViewChange::Claims => {}
    }
    let Some(mut sync) = world.get_mut::<MenuSync>(plan.menu) else {
        return Ok(());
    };
    if let ClickSync::FullContent = sync.state_id.sync_after_click(state_id) {
        queue_full_content(world, plan.menu);
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
            &Menus,
        ),
        With<Slots>,
    >,
    menus: Query<(&MenuLayout, &MenuParts, &MenuButtons, &MenuSync)>,
    holders: Query<(&Slots, Option<&SlotPolicy>)>,
    mut stacks: Stacks,
    mut ops: Local<Operations<SlotRef, StackHandle>>,
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
    let (player_holders, selected, game_mode, drag, player_menus) = players.get(player).ok()?;
    let menu = player_menus.open()?;
    let (layout, parts, buttons, sync) = menus.get(menu).ok()?;
    if container_id != i32::from(sync.container_id) {
        return None;
    }
    let game_mode = game_mode.map_or(GameMode::Survival, |game_mode| game_mode.0);
    let mode = match game_mode {
        GameMode::Spectator => {
            return Some(ClickPlan {
                menu,
                drag: drag.0,
                request: None,
                pressed: None,
                view: ViewChange::FullContent,
            });
        }
        GameMode::Creative => PlayerMode::Creative,
        GameMode::Survival | GameMode::Adventure => PlayerMode::NotCreative,
    };
    if let Clicked::Entry(entry) = click.clicked
        && entry < layout.len()
        && buttons.contains(entry)
    {
        let pressed = (click.kind == ClickKind::PickUp).then_some(EntryPressed {
            viewer: player,
            menu,
            entry,
            button: click.button,
        });
        return Some(ClickPlan {
            menu,
            drag: drag.0,
            request: None,
            pressed,
            view: ViewChange::Claims,
        });
    }
    let menu_slots = MenuSlots { layout, parts };
    let cursor = cursor_slot(player_holders);
    let slots = ClickSlots::read(world, player, menu_slots, cursor, &holders, &mut stacks);
    let clicker = ClickPlayer {
        mode,
        selected: selected.0,
    };
    let click_menu = ClickMenu {
        slots: menu_slots,
        buttons: *buttons,
    };
    let mut drag = drag.0;
    let planned = mcrs_minecraft_inventory_click::request(
        click,
        clicker,
        &click_menu,
        &slots,
        &mut drag,
        &mut ops,
    );
    let request = match planned {
        Err(ClickError::NoSuchEntry) => {
            return Some(ClickPlan {
                menu,
                drag,
                request: None,
                pressed: None,
                view: ViewChange::Unchanged,
            });
        }
        Err(ClickError::NoSuchSlot | ClickError::TooManyOps) => None,
        Ok(()) => engine_request(&ops),
    };
    Some(ClickPlan {
        menu,
        drag,
        request,
        pressed: None,
        view: ViewChange::Claims,
    })
}

fn set_drag(world: &mut World, player: Entity, drag: Drag) {
    if let Some(mut current) = world.get_mut::<ClickDrag>(player) {
        current.set_if_neq(ClickDrag(drag));
    }
}

fn queue_full_content(world: &mut World, menu: Entity) {
    if let Some(mut queue) = world.get_resource_mut::<MenuQueue>() {
        queue.content.push(menu);
    }
}

fn engine_request(ops: &Operations<SlotRef, StackHandle>) -> Option<Request> {
    if ops.is_empty() {
        return None;
    }
    let mut request = Request::new(CLICK_REQUEST);
    for op in ops.iter() {
        request = match op {
            Op::Split { from, to, count } => request.split(from, to, count),
            Op::Merge { from, to } => request.merge(from, to),
            Op::Swap { left, right } => request.swap(left, right),
            Op::Set { slot, stack, count } => {
                request.set(slot, stack.map(|stack| stack.stack.entity), count)
            }
            Op::Drop { from, count } => request.drop(from, count),
        };
    }
    Some(request)
}
