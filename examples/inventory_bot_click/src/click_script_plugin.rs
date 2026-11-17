use bevy_app::App;
use bevy_app::Plugin;
use bevy_app::Update;
use bevy_ecs::prelude::*;
use bevy_log::warn;
use mcrs_minecraft_client_world::player::Player;
use mcrs_minecraft_container_click::Click;
use mcrs_minecraft_container_click::ClickInput;
use mcrs_minecraft_container_click::Clicked;
use mcrs_minecraft_container_click::StackPresence;
use mcrs_minecraft_engine_inventory_menu::MenuParts;
use mcrs_minecraft_engine_inventory_menu::Menus;
use mcrs_minecraft_engine_item::SlotRef;
use mcrs_minecraft_engine_item::Slots;
use mcrs_minecraft_game_mode::Abilities;
use mcrs_minecraft_inventory_client::InventoryClientSet;
use mcrs_minecraft_inventory_client::MenuAction;
use mcrs_minecraft_inventory_client::MenuInput;
use mcrs_minecraft_inventory_client::RemoteMenu;
use mcrs_minecraft_inventory_menu::MenuLayout;
use mcrs_minecraft_network::client::ClientNetworkSystems;
use mcrs_minecraft_player_holders::PlayerHolders;

use crate::ClickScript;
use crate::click_script::ButtonState;

/// Runs the steps of `ClickScript` while the resource exists and the script is not done.
pub struct ClickScriptPlugin;

impl Plugin for ClickScriptPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            click_next_step
                .after(ClientNetworkSystems::Receive)
                .before(InventoryClientSet::Predict)
                .run_if(resource_exists::<ClickScript>.and_then(script_running)),
        );
    }
}

fn script_running(script: Res<ClickScript>) -> bool {
    !script.is_done()
}

fn click_next_step(
    mut script: ResMut<ClickScript>,
    player: Single<(Entity, &PlayerHolders, &Menus, Option<&Abilities>), With<Player>>,
    menus: Query<(&MenuLayout, &MenuParts), With<RemoteMenu>>,
    slots: Query<&Slots>,
    mut inputs: MessageWriter<MenuInput>,
) {
    let (viewer, holders, player_menus, abilities) = *player;
    // The inventory menu is the first menu; the bot clicks only while it is the open one.
    let Some(menu) = player_menus
        .open()
        .filter(|open| player_menus.iter().next() == Some(*open))
    else {
        return;
    };
    // `RemoteMenu` arrives with the first content of the menu, which carries its state id.
    let Ok((layout, parts)) = menus.get(menu) else {
        return;
    };
    let Some(step) = script.next_step() else {
        return;
    };
    let gesture = match script.button() {
        ButtonState::Up => step.press(),
        // A GUI drops the release after a press that clicked, as the player saw the click
        // already happen.
        ButtonState::DownAfterClick => {
            script.release();
            return;
        }
        ButtonState::DownWithoutClick => step.release(),
    };
    let presence = |slot: Option<SlotRef>| {
        let held = slot.and_then(|slot| {
            let holder = slots.get(slot.holder).ok()?;
            holder.get(usize::from(slot.index))
        });
        if held.is_some() {
            StackPresence::Stack
        } else {
            StackPresence::Empty
        }
    };
    let input = ClickInput {
        gesture,
        shift: step.shift,
        control: false,
        creative: abilities.is_some_and(|abilities| abilities.instant_build),
        cursor: presence(Some(SlotRef {
            holder: holders.cursor,
            index: 0,
        })),
        target: presence(layout.entry(step.entry).and_then(|entry| parts.slot(entry))),
    };
    // The same function turns the input of a GUI into a click, so the bot sends what a
    // player with a mouse would send, and nothing for a gesture that clicks nothing.
    let click = Click::from_input(Clicked::Entry(step.entry), input);
    if let Some(click) = click {
        inputs.write(MenuInput {
            viewer,
            action: MenuAction::Click(click),
        });
    }
    match (script.button(), click) {
        (ButtonState::Up, Some(_)) => script.press(ButtonState::DownAfterClick),
        // A press with a stack on the cursor clicks nothing; the release places the stack.
        (ButtonState::Up, None) => script.press(ButtonState::DownWithoutClick),
        (_, None) => {
            warn!(?step, "the step clicks nothing");
            script.release();
        }
        (_, Some(_)) => script.release(),
    }
}
