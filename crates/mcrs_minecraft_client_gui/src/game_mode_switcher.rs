use bevy::math::IVec2;
use bevy::prelude::*;
use bevy::window::{CursorOptions, PrimaryWindow};
use mcrs_minecraft_game_mode::{GameMode, PlayerGameMode, PreviousGameMode};
use mcrs_minecraft_item_icon::Overlays;
use mcrs_minecraft_item_key::item_keys::Item;
use mcrs_minecraft_item_stack::Count;
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_item_stack::Kind;
use mcrs_minecraft_item_stack::NameLookup;
use mcrs_minecraft_item_stack::NameTable;
use mcrs_minecraft_item_table::ItemTable;
use mcrs_minecraft_network::client::{ClientConnection, ClientNetworkSystems};
use mcrs_minecraft_protocol::WritePacket;
use mcrs_minecraft_protocol_game::serverbound::ServerboundChangeGameMode;
use mcrs_minecraft_protocol_game::text::{Color, IntoText, Text};

use crate::cursor_grab::grab;
use crate::debug_screen_overlay::{DebugModifier, toggle_overlay};
use crate::font::{Span, draw_centered_text};
use crate::item_icons::ItemIcons;
use crate::language::ClientLanguage;
use crate::scene::{GuiAtlas, GuiScene, sprite};
use mcrs_minecraft_client_world::chat::ChatMessage;
use mcrs_minecraft_client_world::game_mode::PermissionLevel;
use mcrs_minecraft_client_world::player::Player;
use mcrs_minecraft_client_world::screen::Screen;

const MODIFIER: KeyCode = KeyCode::F3;
const SWITCH: KeyCode = KeyCode::F4;
const SWITCH_KEY_NAME: &str = "key.keyboard.f4";

const ICONS: [GameMode; 4] = [
    GameMode::Creative,
    GameMode::Survival,
    GameMode::Adventure,
    GameMode::Spectator,
];
const ICON_ITEMS: [Item; ICONS.len()] = [
    Item::GrassBlock,
    Item::IronSword,
    Item::BuriedTreasureMap,
    Item::EnderEye,
];
const SLOT_SIZE: i32 = 26;
const ICON_INSET: i32 = 5;
const SLOT_AREA_PADDED: i32 = 31;
const ALL_SLOTS_WIDTH: i32 = ICONS.len() as i32 * SLOT_AREA_PADDED - 5;
const WHITE: u32 = 0xFFFF_FFFF;
const AQUA: u32 = 0xFF55_FFFF;

pub fn next(mode: GameMode) -> GameMode {
    let at = ICONS.iter().position(|&icon| icon == mode).unwrap_or(0);
    ICONS.get((at + 1) % ICONS.len()).copied().unwrap_or(mode)
}

pub fn default_selected(current: GameMode, previous: Option<GameMode>) -> GameMode {
    previous.unwrap_or(if current == GameMode::Creative {
        GameMode::Survival
    } else {
        GameMode::Creative
    })
}

pub fn open_on(
    mode: Option<(&PlayerGameMode, &PreviousGameMode)>,
    permission: Option<&PermissionLevel>,
) -> Option<GameMode> {
    let (current, previous) = mode?;
    (*permission? >= PermissionLevel::GAMEMASTERS).then(|| default_selected(current.0, previous.0))
}

pub fn mode_to_send(
    selected: GameMode,
    mode: Option<&PlayerGameMode>,
    permission: Option<&PermissionLevel>,
) -> Option<GameMode> {
    let mode = mode?;
    (selected != mode.0 && *permission? >= PermissionLevel::GAMEMASTERS).then_some(selected)
}

fn debug_feedback(key: &'static str) -> Text {
    Text::text("")
        .add_child(
            Text::translate("debug.prefix", vec![])
                .color(Color::YELLOW)
                .bold(),
        )
        .add_child(" ".into_text())
        .add_child(Text::translate(key, vec![]))
}

pub struct GameModeSwitcherPlugin;

impl Plugin for GameModeSwitcherPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<ChatMessage>().add_systems(
            Update,
            switch_game_mode
                .after(ClientNetworkSystems::Receive)
                .before(ClientNetworkSystems::Flush)
                .before(toggle_overlay),
        );
    }
}

fn switch_game_mode(
    keys: Res<ButtonInput<KeyCode>>,
    mut screen: ResMut<Screen>,
    mut modifier: ResMut<DebugModifier>,
    player: Query<
        (
            Option<(&PlayerGameMode, &PreviousGameMode)>,
            Option<&PermissionLevel>,
        ),
        With<Player>,
    >,
    mut cursor: Single<&mut CursorOptions, With<PrimaryWindow>>,
    connection: Option<Single<&mut ClientConnection>>,
    mut chat: MessageWriter<ChatMessage>,
) {
    let Ok((mode, permission)) = player.single() else {
        return;
    };
    match *screen {
        Screen::GameModeSwitcher(selected) => {
            if keys.just_released(MODIFIER) {
                if let Some(send) =
                    mode_to_send(selected, mode.map(|(current, _)| current), permission)
                    && let Some(mut connection) = connection
                {
                    connection.write_packet(&ServerboundChangeGameMode { mode: send });
                }
                screen.set_if_neq(Screen::None);
                grab(&mut cursor, true);
            } else if keys.just_pressed(KeyCode::Escape) {
                screen.set_if_neq(Screen::None);
                grab(&mut cursor, true);
            } else if keys.just_pressed(SWITCH) {
                screen.set_if_neq(Screen::GameModeSwitcher(next(selected)));
            }
        }
        Screen::None if keys.pressed(MODIFIER) && keys.just_pressed(SWITCH) && mode.is_some() => {
            modifier.set_if_neq(DebugModifier { used: true });
            match open_on(mode, permission) {
                Some(selected) => {
                    screen.set_if_neq(Screen::GameModeSwitcher(selected));
                    grab(&mut cursor, false);
                }
                None => {
                    chat.write(ChatMessage::system(debug_feedback("debug.gamemodes.error")));
                }
            }
        }
        _ => {}
    }
}

type IconStacks = [Option<ItemStack>; ICONS.len()];

fn icon_stacks(table: &ItemTable) -> IconStacks {
    ICON_ITEMS.map(|item| {
        let id = table.id(NameTable::Item, item.location())?;
        Some(ItemStack::new(Kind(id), Count::new(1)?))
    })
}

pub(crate) fn switcher_shown(screen: Res<Screen>) -> bool {
    matches!(*screen, Screen::GameModeSwitcher(_))
}

pub fn draw(
    screen: Res<Screen>,
    atlas: Option<Res<GuiAtlas>>,
    language: Option<Res<ClientLanguage>>,
    table: Option<Res<ItemTable>>,
    icons: ItemIcons,
    mut stacks: Local<IconStacks>,
    text: Res<SwitcherText>,
    mut scene: ResMut<GuiScene>,
) {
    if let Some(table) = table.filter(DetectChanges::is_changed) {
        *stacks = icon_stacks(&table);
    }
    let Screen::GameModeSwitcher(selected) = *screen else {
        return;
    };
    let (Some(atlas), Some(_)) = (atlas, language) else {
        return;
    };
    switcher(&mut scene, selected, &atlas, &text, &icons, &stacks);
}

#[derive(Resource, Default)]
pub struct SwitcherText {
    names: Vec<(GameMode, Vec<Span>)>,
    hint: Vec<Span>,
}

pub fn rebuild_switcher_text(language: Res<ClientLanguage>, mut text: ResMut<SwitcherText>) {
    text.names.clear();
    for mode in ICONS {
        let name = vec![Span::new(language.get(mode.translation_key()), WHITE)];
        text.names.push((mode, name));
    }
    let key = vec![Span::new(language.get(SWITCH_KEY_NAME), AQUA)];
    let hint = language.translate("debug.gamemodes.select_next", WHITE, &[key]);
    text.map_unchanged(|text| &mut text.hint).set_if_neq(hint);
}

fn switcher(
    scene: &mut GuiScene,
    selected: GameMode,
    atlas: &GuiAtlas,
    text: &SwitcherText,
    icons: &ItemIcons,
    stacks: &IconStacks,
) {
    let font = &atlas.0.font;
    let (cx, cy) = (scene.size.width / 2, scene.size.height / 2);
    scene.quads.push(sprite(
        IVec2::new(cx - 62, cy - SLOT_AREA_PADDED - 27),
        IVec2::new(125, 75),
        "container/gamemode_switcher",
    ));
    let name = text
        .names
        .iter()
        .find(|(mode, _)| *mode == selected)
        .map_or(&[][..], |(_, name)| name.as_slice());
    draw_centered_text(font, cx, cy - SLOT_AREA_PADDED - 20, name, &mut scene.quads);
    draw_centered_text(font, cx, cy + 5, &text.hint, &mut scene.quads);
    for (index, (&mode, stack)) in ICONS.iter().zip(stacks).enumerate() {
        let at = IVec2::new(
            cx - ALL_SLOTS_WIDTH / 2 + index as i32 * SLOT_AREA_PADDED,
            cy - SLOT_AREA_PADDED,
        );
        scene.quads.push(sprite(
            at,
            IVec2::splat(SLOT_SIZE),
            "gamemode_switcher/slot",
        ));
        if mode == selected {
            scene.quads.push(sprite(
                at,
                IVec2::splat(SLOT_SIZE),
                "gamemode_switcher/selection",
            ));
        }
        if let Some(stack) = stack {
            icons.draw_stack(scene, at + ICON_INSET, stack, Overlays::None);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::debug_screen::DebugOverlay;
    use crate::debug_screen_overlay::DebugModifier;
    use bevy::input::common_conditions::input_just_released;
    use bevy::window::CursorGrabMode;

    fn key_app(permission: u8) -> App {
        let mut app = App::new();
        app.init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<Screen>()
            .init_resource::<DebugModifier>()
            .init_resource::<DebugOverlay>()
            .add_message::<ChatMessage>()
            .add_systems(
                Update,
                (
                    switch_game_mode,
                    toggle_overlay.run_if(input_just_released(KeyCode::F3)),
                )
                    .chain(),
            );
        app.world_mut().spawn((
            Player,
            PlayerGameMode(GameMode::Survival),
            PreviousGameMode(None),
            PermissionLevel(permission),
        ));
        app.world_mut().spawn((
            PrimaryWindow,
            CursorOptions {
                grab_mode: CursorGrabMode::Locked,
                ..default()
            },
        ));
        app
    }

    fn press(app: &mut App, key: KeyCode) {
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(key);
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .clear();
    }

    fn release(app: &mut App, key: KeyCode) {
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .release(key);
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .clear();
    }

    fn overlay(app: &App) -> bool {
        **app.world().resource::<DebugOverlay>()
    }

    #[test]
    fn f3_alone_toggles_the_overlay_on_release() {
        let mut app = key_app(0);
        press(&mut app, KeyCode::F3);
        assert!(!overlay(&app));
        release(&mut app, KeyCode::F3);
        assert!(overlay(&app));
    }

    #[test]
    fn f3_f4_cycles_the_switcher_and_leaves_the_overlay_alone() {
        let mut app = key_app(2);
        press(&mut app, KeyCode::F3);
        press(&mut app, KeyCode::F4);
        assert_eq!(
            *app.world().resource::<Screen>(),
            Screen::GameModeSwitcher(GameMode::Creative)
        );
        release(&mut app, KeyCode::F4);
        press(&mut app, KeyCode::F4);
        assert_eq!(
            *app.world().resource::<Screen>(),
            Screen::GameModeSwitcher(GameMode::Survival)
        );
        release(&mut app, KeyCode::F3);
        assert_eq!(*app.world().resource::<Screen>(), Screen::None);
        assert!(!overlay(&app));

        press(&mut app, KeyCode::F3);
        release(&mut app, KeyCode::F3);
        assert!(overlay(&app));
    }

    #[test]
    fn without_permission_f3_f4_reports_instead_of_opening() {
        let mut app = key_app(1);
        press(&mut app, KeyCode::F3);
        press(&mut app, KeyCode::F4);
        release(&mut app, KeyCode::F3);
        assert_eq!(*app.world().resource::<Screen>(), Screen::None);
        assert!(!overlay(&app));
        assert!(!app.world().resource::<Messages<ChatMessage>>().is_empty());
    }

    mod jar {
        use std::sync::Arc;

        use mcrs_minecraft_item_icon::{IconSources, StackOverlay, resolve};
        use mcrs_minecraft_model::item_model_asset::DisplayContext;
        use mcrs_minecraft_model::item_model_context::ItemModelContext;
        use mcrs_minecraft_model::resource_set::ResourceSet;
        use mcrs_minecraft_model::stack_view::StackView;
        use mcrs_minecraft_resource::ResourceLocation;

        use super::*;
        use crate::menu_test_app::{ATLAS, MODELS, TABLE};
        use crate::scene::{GuiQuad, GuiSize, SceneIcon};

        fn expected_icon(name: &str) -> SceneIcon {
            let location = ResourceLocation::read(name).unwrap();
            let kind = Kind(TABLE.id(NameTable::Item, location).unwrap());
            let stack = ItemStack::new(kind, Count::new(1).unwrap());
            let view = StackView {
                stack: &stack,
                table: &TABLE,
            };
            let sources = IconSources {
                models: &MODELS,
                registries: None,
            };
            SceneIcon {
                icon: resolve(view, &sources, &ItemModelContext::new(DisplayContext::Gui)),
                overlay: StackOverlay {
                    count: 1,
                    durability: None,
                },
                overlays: Overlays::None,
            }
        }

        #[test]
        fn the_switcher_draws_the_icon_stack_of_each_entry_over_its_slot() {
            let mut app = App::new();
            app.insert_resource(Screen::GameModeSwitcher(GameMode::Survival))
                .insert_resource(GuiScene {
                    size: GuiSize {
                        width: 427,
                        height: 240,
                        scale: 1,
                    },
                    ..GuiScene::default()
                })
                .insert_resource(GuiAtlas(Arc::clone(&ATLAS)))
                .insert_resource(ClientLanguage::load(ResourceSet::corpus()).unwrap())
                .insert_resource(*TABLE)
                .insert_resource(MODELS.clone())
                .init_resource::<SwitcherText>()
                .add_systems(
                    Update,
                    (
                        rebuild_switcher_text.run_if(resource_exists_and_changed::<ClientLanguage>),
                        draw.run_if(switcher_shown),
                    )
                        .chain(),
                );
            app.update();

            let scene = app.world().resource::<GuiScene>();
            let items = [
                "minecraft:grass_block",
                "minecraft:iron_sword",
                "minecraft:buried_treasure_map",
                "minecraft:ender_eye",
            ];
            assert_eq!(scene.icons, items.map(expected_icon));
            let slot = |x: i32, region| sprite(IVec2::new(x, 89), IVec2::splat(26), region);
            let icon = |x: i32, index| GuiQuad::Icon {
                origin: IVec2::new(x + 5, 94),
                index,
            };
            let drawn: Vec<GuiQuad> = scene
                .quads
                .iter()
                .copied()
                .filter(|quad| match quad {
                    GuiQuad::Sprite { region, .. } => region.starts_with("gamemode_switcher/"),
                    GuiQuad::Icon { .. } => true,
                    _ => false,
                })
                .collect();
            assert_eq!(
                drawn,
                [
                    slot(154, "gamemode_switcher/slot"),
                    icon(154, 0),
                    slot(185, "gamemode_switcher/slot"),
                    slot(185, "gamemode_switcher/selection"),
                    icon(185, 1),
                    slot(216, "gamemode_switcher/slot"),
                    icon(216, 2),
                    slot(247, "gamemode_switcher/slot"),
                    icon(247, 3),
                ]
            );
        }
    }

    fn mode(current: GameMode, previous: Option<GameMode>) -> (PlayerGameMode, PreviousGameMode) {
        (PlayerGameMode(current), PreviousGameMode(previous))
    }

    #[test]
    fn the_switcher_opens_on_the_previous_mode() {
        assert_eq!(
            default_selected(GameMode::Survival, Some(GameMode::Spectator)),
            GameMode::Spectator
        );
        assert_eq!(
            default_selected(GameMode::Creative, None),
            GameMode::Survival
        );
        assert_eq!(
            default_selected(GameMode::Survival, None),
            GameMode::Creative
        );
        assert_eq!(
            default_selected(GameMode::Spectator, None),
            GameMode::Creative
        );
    }

    #[test]
    fn opening_needs_a_joined_game_master() {
        let survival = mode(GameMode::Survival, None);
        assert_eq!(open_on(None, Some(&PermissionLevel(4))), None);
        assert_eq!(open_on(Some((&survival.0, &survival.1)), None), None);
        assert_eq!(
            open_on(Some((&survival.0, &survival.1)), Some(&PermissionLevel(1))),
            None
        );
        assert_eq!(
            open_on(
                Some((&survival.0, &survival.1)),
                Some(&PermissionLevel::GAMEMASTERS)
            ),
            Some(GameMode::Creative)
        );
        assert_eq!(
            open_on(Some((&survival.0, &survival.1)), Some(&PermissionLevel(4))),
            Some(GameMode::Creative)
        );
    }

    #[test]
    fn releasing_sends_only_a_different_mode() {
        let creative = mode(GameMode::Creative, Some(GameMode::Survival));
        let op = PermissionLevel::GAMEMASTERS;
        assert_eq!(
            mode_to_send(GameMode::Creative, Some(&creative.0), Some(&op)),
            None
        );
        assert_eq!(
            mode_to_send(GameMode::Spectator, Some(&creative.0), Some(&op)),
            Some(GameMode::Spectator)
        );
        assert_eq!(
            mode_to_send(
                GameMode::Spectator,
                Some(&creative.0),
                Some(&PermissionLevel(1))
            ),
            None
        );
    }
}
