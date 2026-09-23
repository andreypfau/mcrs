use bevy::math::IVec2;
use bevy::prelude::*;
use bevy::time::Real;
use bevy::window::{CursorOptions, PrimaryWindow};
use mcrs_minecraft_core::codec::Bounded;
use mcrs_minecraft_core::{ResourceKey, ResourceLocation};
use mcrs_minecraft_inventory::{Op, Slot, Transaction};
use mcrs_minecraft_item::{Items, SlotTable};
use mcrs_minecraft_network::client::{ClientConnection, ClientNetworkSystems};
use mcrs_minecraft_protocol::GameMode;
use mcrs_minecraft_protocol::WritePacket;
use mcrs_minecraft_protocol::item::{ComponentPatch, ItemStackValue};
use mcrs_minecraft_protocol::packets::game::serverbound::ServerboundChangeGameMode;

use super::debug_chat::DebugChat;
use super::debug_screen_overlay::{DebugModifier, toggle_overlay};
use super::font::{Span, draw_centered_text};
use super::language::Language;
use super::scene::{GuiAtlas, GuiQuad, GuiScene, sprite};
use crate::game_mode::{LocalGameMode, PermissionLevel};
use crate::inventory::{Screen, grab};
use crate::player::Player;

const MODIFIER: KeyCode = KeyCode::F3;
const SWITCH: KeyCode = KeyCode::F4;
const SWITCH_KEY_NAME: &str = "key.keyboard.f4";

const ICONS: [GameMode; 4] = [
    GameMode::Creative,
    GameMode::Survival,
    GameMode::Adventure,
    GameMode::Spectator,
];
const SLOT_SIZE: i32 = 26;
const SLOT_AREA_PADDED: i32 = 31;
const ALL_SLOTS_WIDTH: i32 = ICONS.len() as i32 * SLOT_AREA_PADDED - 5;
const WHITE: u32 = 0xFFFF_FFFF;
const AQUA: u32 = 0xFF55_FFFF;

fn name_key(mode: GameMode) -> &'static str {
    match mode {
        GameMode::Creative => "gameMode.creative",
        GameMode::Survival => "gameMode.survival",
        GameMode::Adventure => "gameMode.adventure",
        GameMode::Spectator => "gameMode.spectator",
    }
}

fn icon_item(mode: GameMode) -> &'static str {
    match mode {
        GameMode::Creative => "grass_block",
        GameMode::Survival => "iron_sword",
        GameMode::Adventure => "buried_treasure_map",
        GameMode::Spectator => "ender_eye",
    }
}

pub fn next(mode: GameMode) -> GameMode {
    let at = ICONS.iter().position(|&icon| icon == mode).unwrap_or(0);
    ICONS[(at + 1) % ICONS.len()]
}

pub fn default_selected(mode: &LocalGameMode) -> GameMode {
    mode.previous
        .unwrap_or(if mode.current == GameMode::Creative {
            GameMode::Survival
        } else {
            GameMode::Creative
        })
}

/// The selection the switcher opens on, or `None` when the player has not
/// joined or lacks the game-master permission `/gamemode` asks for.
pub fn open_on(
    mode: Option<&LocalGameMode>,
    permission: Option<&PermissionLevel>,
) -> Option<GameMode> {
    let mode = mode?;
    (*permission? >= PermissionLevel::GAMEMASTERS).then(|| default_selected(mode))
}

pub fn mode_to_send(
    selected: GameMode,
    mode: Option<&LocalGameMode>,
    permission: Option<&PermissionLevel>,
) -> Option<GameMode> {
    let mode = mode?;
    (selected != mode.current && *permission? >= PermissionLevel::GAMEMASTERS).then_some(selected)
}

/// The stacks the four slots draw, held by an entity of their own so the item
/// renderer resolves them like any other held stack.
#[derive(Component)]
pub struct GameModeIcons;

pub struct GameModeSwitcherPlugin;

impl Plugin for GameModeSwitcherPlugin {
    fn build(&self, app: &mut App) {
        add_icon_spawning(app);
        app.init_resource::<DebugChat>().add_systems(
                Update,
                switch_game_mode
                    .after(ClientNetworkSystems::Receive)
                    .before(ClientNetworkSystems::Flush)
                    .before(toggle_overlay),
            );
    }
}

/// The item corpus is only known once the client has baked its item models after joining.
fn add_icon_spawning(app: &mut App) {
    app.add_systems(
        Update,
        spawn_icons.run_if(resource_exists::<Items>.and(not(any_with_component::<GameModeIcons>))),
    );
}

fn spawn_icons(world: &mut World) {
    let holder = world
        .spawn((GameModeIcons, SlotTable::fixed(ICONS.len())))
        .id();
    let items = world.resource::<Items>();
    let known = ICONS.iter().all(|&mode| {
        items
            .iter()
            .any(|entry| entry.identifier == ResourceLocation::minecraft(icon_item(mode)))
    });
    if !known {
        warn!("an item the game mode switcher shows is missing from the item corpus");
        return;
    }
    let spawns = ICONS
        .iter()
        .enumerate()
        .map(|(index, &mode)| Op::Spawn {
            value: ItemStackValue {
                item: ResourceKey::from_location(ResourceLocation::minecraft(icon_item(mode))),
                count: Bounded(1),
                components: ComponentPatch::EMPTY,
            },
            to: Slot::new(holder, index as u16),
        })
        .collect();
    if let Err(error) = Transaction(spawns).try_apply(world) {
        warn!("cannot spawn the game mode switcher icons: {error}");
    }
}

#[allow(clippy::too_many_arguments)]
fn switch_game_mode(
    keys: Res<ButtonInput<KeyCode>>,
    mut screen: ResMut<Screen>,
    mut modifier: ResMut<DebugModifier>,
    player: Query<(Option<&LocalGameMode>, Option<&PermissionLevel>), With<Player>>,
    mut cursor: Single<&mut CursorOptions, With<PrimaryWindow>>,
    connection: Option<Single<&mut ClientConnection>>,
    language: Option<Res<Language>>,
    mut chat: ResMut<DebugChat>,
    time: Res<Time<Real>>,
) {
    let Ok((mode, permission)) = player.single() else {
        return;
    };
    match *screen {
        Screen::GameModeSwitcher(selected) => {
            if keys.just_released(MODIFIER) {
                if let Some(send) = mode_to_send(selected, mode, permission)
                    && let Some(mut connection) = connection
                {
                    connection.write_packet(&ServerboundChangeGameMode { mode: send });
                }
                *screen = Screen::None;
                grab(&mut cursor, true);
            } else if keys.just_pressed(KeyCode::Escape) {
                *screen = Screen::None;
                grab(&mut cursor, true);
            } else if keys.just_pressed(SWITCH) {
                *screen = Screen::GameModeSwitcher(next(selected));
            }
        }
        Screen::None if keys.pressed(MODIFIER) && keys.just_pressed(SWITCH) && mode.is_some() => {
            modifier.used = true;
            match open_on(mode, permission) {
                Some(selected) => {
                    *screen = Screen::GameModeSwitcher(selected);
                    grab(&mut cursor, false);
                }
                None => {
                    if let Some(language) = language {
                        chat.feedback(&language, "debug.gamemodes.error", time.elapsed());
                    }
                }
            }
        }
        _ => {}
    }
}

pub fn draw(
    screen: Res<Screen>,
    atlas: Option<Res<GuiAtlas>>,
    language: Option<Res<Language>>,
    icons: Query<&SlotTable, With<GameModeIcons>>,
    mut scene: ResMut<GuiScene>,
) {
    let Screen::GameModeSwitcher(selected) = *screen else {
        return;
    };
    let (Some(atlas), Some(language)) = (atlas, language) else {
        return;
    };
    let scene = &mut *scene;
    switcher(
        scene.size.width,
        scene.size.height,
        selected,
        &atlas,
        &language,
        icons.single().ok(),
        &mut scene.quads,
    );
}

fn switcher(
    width: i32,
    height: i32,
    selected: GameMode,
    atlas: &GuiAtlas,
    language: &Language,
    icons: Option<&SlotTable>,
    out: &mut Vec<GuiQuad>,
) {
    let font = &atlas.0.font;
    let (cx, cy) = (width / 2, height / 2);
    out.push(sprite(
        IVec2::new(cx - 62, cy - SLOT_AREA_PADDED - 27),
        IVec2::new(125, 75),
        "container/gamemode_switcher",
    ));
    let name = [Span::new(language.get(name_key(selected)), WHITE)];
    draw_centered_text(font, cx, cy - SLOT_AREA_PADDED - 20, &name, out);
    let key = vec![Span::new(language.get(SWITCH_KEY_NAME), AQUA)];
    let hint = language.translate("debug.gamemodes.select_next", WHITE, &[key]);
    draw_centered_text(font, cx, cy + 5, &hint, out);
    for (index, &mode) in ICONS.iter().enumerate() {
        let at = IVec2::new(
            cx - ALL_SLOTS_WIDTH / 2 + index as i32 * SLOT_AREA_PADDED,
            cy - SLOT_AREA_PADDED,
        );
        out.push(sprite(
            at,
            IVec2::splat(SLOT_SIZE),
            "gamemode_switcher/slot",
        ));
        if mode == selected {
            out.push(sprite(
                at,
                IVec2::splat(SLOT_SIZE),
                "gamemode_switcher/selection",
            ));
        }
        if let Some(stack) = icons.and_then(|icons| icons.get(index as u16)) {
            out.push(GuiQuad::Item {
                origin: at + 5,
                stack,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gui::debug::DebugScreenEntryList;
    use crate::gui::debug_screen_overlay::DebugModifier;
    use bevy::window::CursorGrabMode;

    fn key_app(permission: u8) -> App {
        let mut app = App::new();
        app.init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<Screen>()
            .init_resource::<DebugModifier>()
            .init_resource::<DebugScreenEntryList>()
            .init_resource::<DebugChat>()
            .init_resource::<Time<Real>>()
            .insert_resource(Language::default())
            .add_systems(Update, (switch_game_mode, toggle_overlay).chain());
        app.world_mut().spawn((
            Player,
            LocalGameMode {
                current: GameMode::Survival,
                previous: None,
            },
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
        app.world()
            .resource::<DebugScreenEntryList>()
            .overlay_visible()
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
        assert!(app.world().resource::<DebugChat>().has_messages());
    }

    fn mode(current: GameMode, previous: Option<GameMode>) -> LocalGameMode {
        LocalGameMode { current, previous }
    }

    #[test]
    fn the_four_icon_stacks_are_held_in_order() {
        let mut world = World::new();
        world.insert_resource(mcrs_minecraft_item::test_corpus().1.clone());
        spawn_icons(&mut world);
        let mut holders = world.query_filtered::<&SlotTable, With<GameModeIcons>>();
        let icons = holders.single(&world).unwrap();
        let held: Vec<_> = (0..4)
            .map(|index| {
                let stack = icons.get(index).expect("an icon in every slot");
                mcrs_minecraft_item::value::entry(&world, stack, world.resource::<Items>())
                    .unwrap()
                    .identifier
                    .to_string()
            })
            .collect();
        assert_eq!(
            held,
            [
                "minecraft:grass_block",
                "minecraft:iron_sword",
                "minecraft:buried_treasure_map",
                "minecraft:ender_eye"
            ]
        );
    }

    #[test]
    fn the_icons_appear_once_the_item_corpus_arrives_after_startup() {
        let mut app = App::new();
        add_icon_spawning(&mut app);
        app.update();
        let mut holders = app
            .world_mut()
            .query_filtered::<&SlotTable, With<GameModeIcons>>();
        assert_eq!(holders.iter(app.world()).count(), 0);

        app.insert_resource(mcrs_minecraft_item::test_corpus().1.clone());
        app.update();
        app.update();
        let icons: Vec<_> = holders.iter(app.world()).collect();
        assert_eq!(icons.len(), 1);
        assert!((0..4).all(|index| icons[0].get(index).is_some()));
    }

    #[test]
    fn each_press_moves_to_the_next_icon_and_wraps() {
        assert_eq!(next(GameMode::Creative), GameMode::Survival);
        assert_eq!(next(GameMode::Survival), GameMode::Adventure);
        assert_eq!(next(GameMode::Adventure), GameMode::Spectator);
        assert_eq!(next(GameMode::Spectator), GameMode::Creative);
    }

    #[test]
    fn the_switcher_opens_on_the_previous_mode() {
        assert_eq!(
            default_selected(&mode(GameMode::Survival, Some(GameMode::Spectator))),
            GameMode::Spectator
        );
        assert_eq!(
            default_selected(&mode(GameMode::Creative, None)),
            GameMode::Survival
        );
        assert_eq!(
            default_selected(&mode(GameMode::Survival, None)),
            GameMode::Creative
        );
        assert_eq!(
            default_selected(&mode(GameMode::Spectator, None)),
            GameMode::Creative
        );
    }

    #[test]
    fn opening_needs_a_joined_game_master() {
        let survival = mode(GameMode::Survival, None);
        assert_eq!(open_on(None, Some(&PermissionLevel(4))), None);
        assert_eq!(open_on(Some(&survival), None), None);
        assert_eq!(open_on(Some(&survival), Some(&PermissionLevel(1))), None);
        assert_eq!(
            open_on(Some(&survival), Some(&PermissionLevel::GAMEMASTERS)),
            Some(GameMode::Creative)
        );
        assert_eq!(
            open_on(Some(&survival), Some(&PermissionLevel(4))),
            Some(GameMode::Creative)
        );
    }

    #[test]
    fn releasing_sends_only_a_different_mode() {
        let creative = mode(GameMode::Creative, Some(GameMode::Survival));
        let op = PermissionLevel::GAMEMASTERS;
        assert_eq!(
            mode_to_send(GameMode::Creative, Some(&creative), Some(&op)),
            None
        );
        assert_eq!(
            mode_to_send(GameMode::Spectator, Some(&creative), Some(&op)),
            Some(GameMode::Spectator)
        );
        assert_eq!(
            mode_to_send(
                GameMode::Spectator,
                Some(&creative),
                Some(&PermissionLevel(1))
            ),
            None
        );
    }
}
