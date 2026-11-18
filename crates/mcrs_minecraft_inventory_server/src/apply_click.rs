use bevy_ecs::entity::Entity;
use bevy_ecs::query::With;
use bevy_ecs::system::InRef;
use bevy_ecs::system::Query;
use bevy_ecs::system::RegisteredSystemError;
use bevy_ecs::system::Res;
use bevy_ecs::world::World;
use mcrs_minecraft_container_click::ClickKind;
use mcrs_minecraft_container_click::Clicked;
use mcrs_minecraft_engine_inventory_click::MenuClickError;
use mcrs_minecraft_engine_inventory_click::MenuClicker;
use mcrs_minecraft_engine_inventory_click::MenuClicks;
use mcrs_minecraft_engine_inventory_menu::EntryPressed;
use mcrs_minecraft_engine_inventory_menu::MenuButtons;
use mcrs_minecraft_engine_inventory_menu::Menus;
use mcrs_minecraft_engine_item::HolderCommandsExt;
use mcrs_minecraft_engine_item::Request;
use mcrs_minecraft_engine_item::RequestId;
use mcrs_minecraft_engine_item::Selected;
use mcrs_minecraft_engine_item::Slots;
use mcrs_minecraft_game_mode::{GameMode, PlayerGameMode};
use mcrs_minecraft_inventory_click::ClickError;
use mcrs_minecraft_inventory_click::ClickPlayer;
use mcrs_minecraft_inventory_client_view::ClickSync;
use mcrs_minecraft_inventory_menu::MenuLayout;
use mcrs_minecraft_inventory_player::PlayerMode;
use mcrs_minecraft_player_holders::PlayerHolders;
use mcrs_minecraft_vanilla_inventory::RequestPurpose;
use mcrs_minecraft_vanilla_inventory::VanillaInventoryRequests;

use crate::InventoryInput;
use crate::MenuSync;
use crate::cursor_slot::cursor_slot;
use crate::menu_queue::MenuQueue;

pub(crate) struct ClickPlan {
    menu: Entity,
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
    players: Query<(&PlayerHolders, &Selected, Option<&PlayerGameMode>, &Menus), With<Slots>>,
    menus: Query<(&MenuLayout, &MenuButtons, &MenuSync)>,
    requests: Option<Res<VanillaInventoryRequests>>,
    mut clicks: MenuClicks,
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
    let (player_holders, selected, game_mode, player_menus) = players.get(player).ok()?;
    let menu = player_menus.open()?;
    let (layout, buttons, sync) = menus.get(menu).ok()?;
    if container_id != i32::from(sync.container_id) {
        return None;
    }
    let game_mode = game_mode.map_or(GameMode::Survival, |game_mode| game_mode.0);
    let mode = match game_mode {
        GameMode::Spectator => {
            return Some(ClickPlan {
                menu,
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
            request: None,
            pressed,
            view: ViewChange::Claims,
        });
    }
    let Some(requests) = requests else {
        return Some(ClickPlan {
            menu,
            request: None,
            pressed: None,
            view: ViewChange::FullContent,
        });
    };
    let source = requests.source;
    let clicker = MenuClicker {
        player: ClickPlayer {
            mode,
            selected: selected.0,
        },
        cursor: cursor_slot(player_holders),
        request: RequestId {
            source,
            serial: RequestPurpose::Click.serial(),
        },
    };
    let (request, view) = match clicks.plan(player, click, clicker) {
        Ok(request) if request.ops().next().is_none() => (None, ViewChange::Claims),
        Ok(request) => (Some(request), ViewChange::Claims),
        Err(MenuClickError::NoOpenMenu | MenuClickError::NoMenuDrag) => return None,
        Err(MenuClickError::Click(ClickError::NoSuchEntry)) => (None, ViewChange::Unchanged),
        Err(MenuClickError::Click(ClickError::NoSuchSlot | ClickError::TooManyOps)) => {
            (None, ViewChange::Claims)
        }
    };
    Some(ClickPlan {
        menu,
        request,
        pressed: None,
        view,
    })
}

fn queue_full_content(world: &mut World, menu: Entity) {
    if let Some(mut queue) = world.get_resource_mut::<MenuQueue>() {
        queue.content.push(menu);
    }
}
