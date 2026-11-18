use std::sync::{Arc, LazyLock};
use std::time::Duration;

use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::input::mouse::MouseButtonInput;
use bevy::input::{ButtonState, InputPlugin};
use bevy::prelude::*;
use bevy::time::Real;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};
use mcrs_minecraft_client_options::ClientOptions;
use mcrs_minecraft_client_widget::{GuiExtent, WidgetId};
use mcrs_minecraft_client_world::player::Player;
use mcrs_minecraft_client_world::screen::Screen;
use mcrs_minecraft_client_world::world_list::{ReadWorldList, WorldList};
use mcrs_minecraft_engine_registries::test_item_table;
use mcrs_minecraft_item_table::ItemTable;
use mcrs_minecraft_model::atlas::SpriteRegistry;
use mcrs_minecraft_model::item_model_bake::{ItemModels, bake_all};
use mcrs_minecraft_model::resource_set::ResourceSet;
use mcrs_minecraft_resource::ResourceLocationBuf;
use mcrs_minecraft_save::{ListedLevel, WorldSummary};
use time::UtcOffset;

use crate::language::ClientLanguage;
use crate::menu::Menu;
use crate::menu_plugin::MenuPlugin;
use crate::menu_screens_plugin::MenuScreensPlugin;
use crate::scene::{GuiAtlas, GuiAtlasData, GuiConfig, GuiScene, GuiSize};

pub(crate) static ATLAS: LazyLock<Arc<GuiAtlasData>> =
    LazyLock::new(|| Arc::new(GuiAtlasData::load(ResourceSet::corpus()).unwrap()));

pub(crate) static TABLE: LazyLock<ItemTable> = LazyLock::new(test_item_table);

pub(crate) static MODELS: LazyLock<ItemModels> =
    LazyLock::new(|| bake_all(ResourceSet::corpus(), &mut SpriteRegistry::default()).unwrap());

pub(crate) const GUI: GuiExtent = GuiExtent::new(427, 240);

pub(crate) fn game_app() -> App {
    let mut app = App::new();
    app.add_plugins((
        InputPlugin,
        MenuPlugin,
        MenuScreensPlugin {
            preselected_world: None,
            utc_offset: UtcOffset::UTC,
            singleplayer: true,
        },
    ))
    .insert_resource(GuiConfig {
        scale: 1,
        cursor: Some(IVec2::ZERO),
        frozen_ticks: None,
        advanced_tooltips: false,
    })
    .insert_resource(GuiScene {
        size: GuiSize {
            width: GUI.width,
            height: GUI.height,
            scale: 1,
        },
        ..GuiScene::default()
    })
    .insert_resource(GuiAtlas(Arc::clone(&ATLAS)))
    .insert_resource(ClientLanguage::load(ResourceSet::corpus()).unwrap())
    .init_resource::<ClientOptions>()
    .init_resource::<Screen>()
    .init_resource::<Time<Real>>();
    app.world_mut().spawn((Window::default(), PrimaryWindow));
    app.world_mut().spawn(Player);
    app.update();
    app
}

pub(crate) fn tap_escape(app: &mut App) {
    for state in [ButtonState::Pressed, ButtonState::Released] {
        app.world_mut().write_message(KeyboardInput {
            key_code: KeyCode::Escape,
            logical_key: Key::Escape,
            state,
            text: None,
            repeat: false,
            window: Entity::PLACEHOLDER,
        });
        app.update();
    }
}

pub(crate) fn click(app: &mut App, x: i32, y: i32) {
    app.world_mut().resource_mut::<GuiConfig>().cursor = Some(IVec2::new(x, y));
    for state in [ButtonState::Pressed, ButtonState::Released] {
        app.world_mut().write_message(MouseButtonInput {
            button: MouseButton::Left,
            state,
            window: Entity::PLACEHOLDER,
        });
        app.update();
    }
}

pub(crate) fn screen(app: &App) -> Screen {
    *app.world().resource::<Screen>()
}

pub(crate) fn grab_mode(app: &mut App) -> CursorGrabMode {
    app.world_mut()
        .query_filtered::<&CursorOptions, With<PrimaryWindow>>()
        .single(app.world())
        .unwrap()
        .grab_mode
}

pub(crate) fn caption(app: &App, widget: WidgetId) -> String {
    let menu = app.world().resource::<Menu>();
    menu.widgets.get(widget).unwrap().caption.clone()
}

pub(crate) fn advance(app: &mut App, ms: u64) {
    app.world_mut()
        .resource_mut::<Time<Real>>()
        .update_with_duration(Duration::from_millis(ms));
    app.update();
}

pub(crate) fn title_app() -> App {
    let mut app = game_app();
    let player = app
        .world_mut()
        .query_filtered::<Entity, With<Player>>()
        .single(app.world())
        .unwrap();
    app.world_mut().despawn(player);
    *app.world_mut().resource_mut::<Screen>() = Screen::Title;
    app.update();
    app
}

pub(crate) fn world(folder_id: &str, name: &str, locked: bool, world_version: i32) -> WorldSummary {
    WorldSummary {
        folder_id: folder_id.to_owned(),
        last_played: Some(1_794_668_820_000),
        locked,
        level: Some(ListedLevel {
            level_name: name.to_owned(),
            version_name: "1.0".to_owned(),
            world_version,
            storage_version: 19133,
            game_type: 0,
            hardcore: false,
            allow_commands: false,
            experimental: false,
            host: None,
        }),
    }
}

pub(crate) fn world_types() -> Vec<ResourceLocationBuf> {
    ["minecraft:normal", "minecraft:flat", "minecraft:amplified"]
        .map(|name| ResourceLocationBuf::read(name).unwrap())
        .to_vec()
}

pub(crate) fn press(app: &mut App, key_code: KeyCode, logical_key: Key, text: Option<&str>) {
    for state in [ButtonState::Pressed, ButtonState::Released] {
        app.world_mut().write_message(KeyboardInput {
            key_code,
            logical_key: logical_key.clone(),
            state,
            text: text
                .filter(|_| state == ButtonState::Pressed)
                .map(Into::into),
            repeat: false,
            window: Entity::PLACEHOLDER,
        });
        app.update();
    }
}

pub(crate) fn type_text(app: &mut App, text: &str) {
    for letter in text.chars() {
        let letter = letter.to_string();
        let key = Key::Character(letter.as_str().into());
        press(app, KeyCode::KeyA, key, Some(&letter));
    }
}

pub(crate) fn drained<M: Message>(app: &mut App) -> Vec<M> {
    app.world_mut()
        .resource_mut::<Messages<M>>()
        .drain()
        .collect()
}

pub(crate) fn read_like_the_store(
    mut reads: MessageReader<ReadWorldList>,
    mut list: ResMut<WorldList>,
) {
    if reads.read().count() > 0 {
        list.set_if_neq(WorldList::Reading);
    }
}
