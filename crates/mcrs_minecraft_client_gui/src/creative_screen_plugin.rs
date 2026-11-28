use bevy::ecs::system::SystemParam;
use bevy::input::keyboard::KeyboardInput;
use bevy::input::{ButtonState, InputSystems};
use bevy::prelude::*;
use mcrs_minecraft_client_options::ClientOptions;
use mcrs_minecraft_client_world::game_mode::PermissionLevel;
use mcrs_minecraft_client_world::player::Player;
use mcrs_minecraft_creative_content::OperatorCondition;
use mcrs_minecraft_game_mode::Abilities;
use mcrs_minecraft_item_table::ItemTable;
use mcrs_minecraft_menu_geometry::CreativeTabKind;
use mcrs_minecraft_network::client::ClientNetworkSystems;
use mcrs_minecraft_registries::CurrentRegistries;

use crate::creative_screen_open::open_creative_screen;
use crate::creative_screen_shown::creative_screen_shown;
use crate::creative_screen_state::CreativeScreenState;
use crate::creative_tab_content::CreativeInputs;
use crate::creative_tab_content::CreativeTabContent;
use crate::language::ClientLanguage;
use crate::loaded_creative_report::LoadedCreativeReport;
use crate::menu_input;

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CreativeContentSystems;

pub struct CreativeScreenPlugin;

impl Plugin for CreativeScreenPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(LoadedCreativeReport::embedded())
            .init_resource::<CreativeTabContent>()
            .init_resource::<CreativeScreenState>()
            .add_observer(open_creative_screen)
            .add_systems(
                PreUpdate,
                capture_search_keys
                    .after(InputSystems)
                    .run_if(resource_exists::<ButtonInput<KeyCode>>)
                    .run_if(creative_screen_shown),
            )
            .add_systems(
                Update,
                (
                    rebuild_creative_content.run_if(creative_inputs_changed),
                    follow_creative_content
                        .run_if(resource_changed::<CreativeTabContent>)
                        .run_if(creative_screen_shown),
                )
                    .chain()
                    .in_set(CreativeContentSystems)
                    .after(ClientNetworkSystems::Receive),
            );
    }
}

#[derive(SystemParam)]
struct CreativeSession<'w, 's> {
    registries: Option<Res<'w, CurrentRegistries>>,
    items: Option<Res<'w, ItemTable>>,
    language: Option<Res<'w, ClientLanguage>>,
    options: Option<Res<'w, ClientOptions>>,
    player:
        Query<'w, 's, (Option<&'static Abilities>, Option<&'static PermissionLevel>), With<Player>>,
}

impl CreativeSession<'_, '_> {
    fn operator(&self) -> OperatorCondition {
        let Ok((abilities, permission)) = self.player.single() else {
            return OperatorCondition::NotMet;
        };
        let creative = abilities.is_some_and(|abilities| abilities.instant_build);
        let gamemaster = permission.is_some_and(|level| *level >= PermissionLevel::GAMEMASTERS);
        let tab_on = self
            .options
            .as_ref()
            .is_some_and(|options| options.operator_items_tab);
        if creative && gamemaster && tab_on {
            OperatorCondition::Met
        } else {
            OperatorCondition::NotMet
        }
    }
}

#[cfg_attr(mcrs_lint, mcrs::event_condition)]
fn creative_inputs_changed(session: CreativeSession, content: Res<CreativeTabContent>) -> bool {
    let (Some(registries), Some(items)) = (&session.registries, &session.items) else {
        return false;
    };
    registries.is_changed()
        || items.is_changed()
        || session
            .language
            .as_ref()
            .is_some_and(|language| language.is_changed())
        || session.operator() != content.operator()
}

fn rebuild_creative_content(
    session: CreativeSession,
    report: Res<LoadedCreativeReport>,
    mut content: ResMut<CreativeTabContent>,
) {
    let (Some(registries), Some(items)) = (&session.registries, &session.items) else {
        return;
    };
    *content = CreativeTabContent::build(CreativeInputs {
        report: report.0.as_ref(),
        registries,
        items,
        language: session.language.as_deref(),
        operator: session.operator(),
    });
}

fn capture_search_keys(
    screen: Res<CreativeScreenState>,
    mut keyboard: MessageReader<KeyboardInput>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
) {
    if screen.selected_tab().kind != CreativeTabKind::Search {
        keyboard.clear();
        return;
    }
    for key in keyboard.read() {
        if key.state == ButtonState::Pressed {
            menu_input::capture_key(&mut keys, key.key_code);
        }
    }
}

fn follow_creative_content(
    content: Res<CreativeTabContent>,
    mut screen: ResMut<CreativeScreenState>,
) {
    screen.content_changed(&content);
}
