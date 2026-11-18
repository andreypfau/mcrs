use std::sync::LazyLock;

use bevy::input::ButtonState;
use bevy::input::keyboard::Key;
use bevy::input::keyboard::KeyboardInput;
use bevy::input::keyboard::NativeKey;
use bevy::prelude::*;
use mcrs_minecraft_client_options::ClientOptions;
use mcrs_minecraft_client_widget::KeyModifiers;
use mcrs_minecraft_client_world::game_mode::PermissionLevel;
use mcrs_minecraft_client_world::player::Player;
use mcrs_minecraft_client_world::screen::Screen;
use mcrs_minecraft_creative_content::OperatorCondition;
use mcrs_minecraft_creative_content::SearchQuery;
use mcrs_minecraft_creative_report::CreativeReport;
use mcrs_minecraft_engine_inventory_menu::MenuOf;
use mcrs_minecraft_game_mode::Abilities;
use mcrs_minecraft_game_mode::GameMode;
use mcrs_minecraft_game_registries::registry_load::test_registries;
use mcrs_minecraft_item_stack::Count;
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_item_stack::Kind;
use mcrs_minecraft_item_stack::NameLookup;
use mcrs_minecraft_item_stack::NameTable;
use mcrs_minecraft_item_stack::Profile;
use mcrs_minecraft_item_table::ItemTable;
use mcrs_minecraft_menu_geometry::CREATIVE_TABS;
use mcrs_minecraft_menu_geometry::CreativeGeometry;
use mcrs_minecraft_menu_geometry::CreativeTabIndex;
use mcrs_minecraft_menu_geometry::CreativeTabKind;
use mcrs_minecraft_model::resource_set::ResourceSet;
use mcrs_minecraft_registries::CurrentRegistries;
use mcrs_minecraft_resource::rl;

use crate::creative_screen_plugin::CreativeContentSystems;
use crate::creative_screen_plugin::CreativeScreenPlugin;
use crate::creative_screen_state::CreativeScreenState;
use crate::creative_search_edit;
use crate::creative_search_index::CreativeSearchIndex;
use crate::creative_tab_content::CreativeInputs;
use crate::creative_tab_content::CreativeTabContent;
use crate::language::ClientLanguage;
use crate::loaded_creative_report::LoadedCreativeReport;
use crate::mark_creative_screen::mark_creative_screen;
use crate::menu_test_app::TABLE;
use crate::search_input::SearchInput;

const BUILDING_BLOCKS: CreativeTabIndex = tab(0);
const COLORED_BLOCKS: CreativeTabIndex = tab(1);
const SAVED_HOTBARS: CreativeTabIndex = tab(5);
const SEARCH: CreativeTabIndex = tab(6);
const OP_BLOCKS: CreativeTabIndex = tab(12);
const SURVIVAL_INVENTORY: CreativeTabIndex = tab(13);

const fn tab(index: usize) -> CreativeTabIndex {
    match CreativeTabIndex::new(index) {
        Some(tab) => tab,
        None => panic!("the creative tabs hold the index"),
    }
}

static REPORT: LazyLock<LoadedCreativeReport> = LazyLock::new(LoadedCreativeReport::embedded);

pub(crate) static VANILLA: LazyLock<CreativeTabContent> =
    LazyLock::new(|| vanilla(OperatorCondition::NotMet));

fn vanilla(operator: OperatorCondition) -> CreativeTabContent {
    CreativeTabContent::build(CreativeInputs {
        report: Some(
            REPORT
                .0
                .as_ref()
                .expect("the stored creative report parses"),
        ),
        registries: test_registries(),
        items: &TABLE,
        language: Some(ClientLanguage::corpus()),
        operator,
    })
}

fn sized(building_blocks: usize) -> CreativeTabContent {
    let names = test_registries()
        .names_of(rl!("minecraft:item"))
        .expect("the item registry exists");
    let stacks: Vec<String> = names
        .iter()
        .filter(|name| name.as_location() != "minecraft:air")
        .take(building_blocks)
        .map(|name| format!(r#"{{ "id": "{name}" }}"#))
        .collect();
    assert_eq!(
        stacks.len(),
        building_blocks,
        "the registry has enough items"
    );
    let stacks = stacks.join(", ");
    let tabs: Vec<String> = CREATIVE_TABS
        .iter()
        .filter(|tab| tab.kind == CreativeTabKind::Items)
        .map(|tab| {
            let list = if tab.id == "minecraft:building_blocks" {
                stacks.as_str()
            } else {
                ""
            };
            format!(
                r#""{}": {{ "needs_operator": false, "tab_list": [{list}], "search_list": [] }}"#,
                tab.id
            )
        })
        .collect();
    let report = format!(
        r#"{{ "item_features": {{}}, "tabs": {{ {} }} }}"#,
        tabs.join(", ")
    );
    let report = CreativeReport::read(report.as_bytes()).expect("the report parses");
    CreativeTabContent::build(CreativeInputs {
        report: Some(&report),
        registries: test_registries(),
        items: &TABLE,
        language: Some(ClientLanguage::corpus()),
        operator: OperatorCondition::NotMet,
    })
}

fn opened(content: &CreativeTabContent) -> CreativeScreenState {
    let mut state = CreativeScreenState::default();
    state.content_changed(content);
    state.open(content);
    state
}

fn item_of(stack: &ItemStack) -> String {
    TABLE
        .name(NameTable::Item, stack.kind.0)
        .map(|name| name.to_string())
        .unwrap_or_default()
}

fn assert_scroll(actual: f64, expected: f64, case: &str) {
    assert!(
        (actual - expected).abs() < 1e-9,
        "{case}: the scroll offset {actual} is not {expected}"
    );
}

struct ScrollRow {
    len: usize,
    wheel: &'static [f64],
    scroll: f64,
    slots: &'static [(u16, Option<usize>)],
}

#[test]
fn the_wheel_scrolls_by_rows_only_when_the_content_exceeds_the_grid() {
    let rows = [
        ScrollRow {
            len: 45,
            wheel: &[-1.0],
            scroll: 0.0,
            slots: &[(0, Some(0)), (44, Some(44))],
        },
        ScrollRow {
            len: 46,
            wheel: &[-1.0],
            scroll: 1.0,
            slots: &[(0, Some(9)), (36, Some(45)), (37, None)],
        },
        ScrollRow {
            len: 100,
            wheel: &[-1.0],
            scroll: 1.0 / 7.0,
            slots: &[(0, Some(9))],
        },
        ScrollRow {
            len: 100,
            wheel: &[-10.0],
            scroll: 1.0,
            slots: &[(0, Some(63)), (36, Some(99)), (37, None), (45, None)],
        },
        ScrollRow {
            len: 100,
            wheel: &[-1.0, 2.0],
            scroll: 0.0,
            slots: &[(0, Some(0))],
        },
    ];
    for row in rows {
        let content = sized(row.len);
        let mut state = opened(&content);
        for vertical in row.wheel {
            state.wheel(*vertical, &content);
        }
        let case = format!("{} stacks, wheel {:?}", row.len, row.wheel);
        assert_scroll(state.scroll(), row.scroll, &case);
        let list = content.tab_list(BUILDING_BLOCKS);
        for (slot, index) in row.slots {
            let expected = index.and_then(|index| list.get(index));
            assert_eq!(
                state.grid_stack(*slot, &content),
                expected,
                "{case}: grid slot {slot}"
            );
        }
    }
}

struct ChangeRow {
    before: usize,
    wheel: f64,
    after: usize,
    first_row: i32,
    scroll: f64,
}

#[test]
fn a_change_of_the_content_keeps_the_first_visible_row() {
    let rows = [
        ChangeRow {
            before: 100,
            wheel: -3.0,
            after: 200,
            first_row: 3,
            scroll: 3.0 / 18.0,
        },
        ChangeRow {
            before: 100,
            wheel: -3.0,
            after: 46,
            first_row: 1,
            scroll: 1.0,
        },
        ChangeRow {
            before: 100,
            wheel: -7.0,
            after: 100,
            first_row: 7,
            scroll: 1.0,
        },
        ChangeRow {
            before: 100,
            wheel: -3.0,
            after: 40,
            first_row: 0,
            scroll: 0.0,
        },
    ];
    for row in rows {
        let before = sized(row.before);
        let after = sized(row.after);
        let mut state = opened(&before);
        state.wheel(row.wheel, &before);
        state.content_changed(&after);
        let case = format!("{} to {} stacks", row.before, row.after);
        assert_eq!(
            state.selected(),
            BUILDING_BLOCKS,
            "{case}: the selected tab"
        );
        assert_eq!(
            state.first_row(&after),
            row.first_row,
            "{case}: the first row"
        );
        assert_scroll(state.scroll(), row.scroll, &case);
    }
}

struct SelectRow {
    name: &'static str,
    steps: fn(&mut CreativeScreenState, &CreativeTabContent),
    selected: CreativeTabIndex,
    text: &'static str,
}

#[test]
fn a_selection_resets_the_scroll_and_the_search_text() {
    let rows = [
        SelectRow {
            name: "another items tab",
            steps: |state, content| {
                state.select(SEARCH, content);
                state.edit_search(content, |search| {
                    search.insert("st");
                });
                state.select(BUILDING_BLOCKS, content);
            },
            selected: BUILDING_BLOCKS,
            text: "",
        },
        SelectRow {
            name: "the search tab again",
            steps: |state, content| {
                state.select(SEARCH, content);
                state.edit_search(content, |search| {
                    search.insert("st");
                });
                state.wheel(-1.0, content);
                state.select(SEARCH, content);
            },
            selected: SEARCH,
            text: "st",
        },
        SelectRow {
            name: "the same items tab",
            steps: |state, content| {
                state.wheel(-3.0, content);
                state.select(BUILDING_BLOCKS, content);
            },
            selected: BUILDING_BLOCKS,
            text: "",
        },
        SelectRow {
            name: "an open keeps the tab and clears the search",
            steps: |state, content| {
                state.select(SEARCH, content);
                state.edit_search(content, |search| {
                    search.insert("st");
                });
                state.open(content);
            },
            selected: SEARCH,
            text: "",
        },
        SelectRow {
            name: "the saved hotbars tab",
            steps: |state, content| {
                state.wheel(-3.0, content);
                state.select(SAVED_HOTBARS, content);
            },
            selected: SAVED_HOTBARS,
            text: "",
        },
        SelectRow {
            name: "the hidden operator tab",
            steps: |state, content| {
                state.select(OP_BLOCKS, content);
            },
            selected: BUILDING_BLOCKS,
            text: "",
        },
    ];
    for row in rows {
        let mut state = opened(&VANILLA);
        (row.steps)(&mut state, &VANILLA);
        assert_eq!(
            state.selected(),
            row.selected,
            "{}: the selected tab",
            row.name
        );
        assert_eq!(
            state.search_box().text(),
            row.text,
            "{}: the search text",
            row.name
        );
        assert_scroll(state.scroll(), 0.0, row.name);
    }
}

struct SearchKeyRow {
    name: &'static str,
    keys: &'static [SearchKey],
    text: &'static str,
    scrolled: bool,
    copied: Option<&'static str>,
}

#[derive(Clone, Copy)]
struct SearchKey {
    code: KeyCode,
    text: Option<&'static str>,
    modifiers: KeyModifiers,
}

const fn typed(code: KeyCode, text: &'static str) -> SearchKey {
    SearchKey {
        code,
        text: Some(text),
        modifiers: KeyModifiers::NONE,
    }
}

const fn shortcut(code: KeyCode) -> SearchKey {
    SearchKey {
        code,
        text: None,
        modifiers: KeyModifiers::CONTROL,
    }
}

const W: SearchKey = typed(KeyCode::KeyW, "w");
const SELECT_ALL: SearchKey = shortcut(KeyCode::KeyA);
const COPY: SearchKey = shortcut(KeyCode::KeyC);
const CUT: SearchKey = shortcut(KeyCode::KeyX);

#[test]
fn a_search_key_keeps_the_caret_in_view_and_copies_only_a_selection() {
    let rows = [
        SearchKeyRow {
            name: "twenty wide characters",
            keys: &[W; 20],
            text: "wwwwwwwwwwwwwwwwwwww",
            scrolled: true,
            copied: None,
        },
        SearchKeyRow {
            name: "a short text",
            keys: &[W; 2],
            text: "ww",
            scrolled: false,
            copied: None,
        },
        SearchKeyRow {
            name: "a copy without a selection",
            keys: &[W, COPY],
            text: "w",
            scrolled: false,
            copied: None,
        },
        SearchKeyRow {
            name: "a cut without a selection",
            keys: &[W, CUT],
            text: "w",
            scrolled: false,
            copied: None,
        },
        SearchKeyRow {
            name: "a cut of the whole text",
            keys: &[W, W, SELECT_ALL, CUT],
            text: "",
            scrolled: false,
            copied: Some("ww"),
        },
    ];
    let width = |text: &str| 6 * i32::try_from(text.chars().count()).unwrap();
    for row in rows {
        let mut state = opened(&VANILLA);
        state.select(SEARCH, &VANILLA);
        let mut copied = None;
        for key in row.keys {
            let input = SearchInput::Key {
                input: KeyboardInput {
                    key_code: key.code,
                    logical_key: Key::Unidentified(NativeKey::Unidentified),
                    state: ButtonState::Pressed,
                    text: key.text.map(Into::into),
                    repeat: false,
                    window: Entity::PLACEHOLDER,
                },
                modifiers: key.modifiers,
            };
            state.edit_search(&VANILLA, |edit| {
                copied = creative_search_edit::apply_search(
                    edit,
                    &input,
                    CreativeGeometry::SEARCH_BOX,
                    None,
                    &width,
                    || None,
                )
                .or(copied.take());
            });
        }
        let search = state.search_box();
        assert_eq!(search.text(), row.text, "{}: the search text", row.name);
        assert_eq!(
            copied.as_deref(),
            row.copied,
            "{}: the copied text",
            row.name
        );
        let first = search
            .layout(CreativeGeometry::SEARCH_BOX, 0, &width)
            .visible
            .start;
        assert_eq!(
            first > 0,
            row.scrolled,
            "{}: the first visible position {first}",
            row.name
        );
    }
}

struct SearchRow {
    text: &'static str,
    found: &'static [&'static str],
    not_found: &'static [&'static str],
}

#[test]
fn the_search_text_filters_the_search_list_by_name_id_and_tag() {
    let rows = [
        SearchRow {
            text: "",
            found: &["minecraft:stone", "minecraft:diamond_sword"],
            not_found: &[],
        },
        SearchRow {
            text: "DiaMond",
            found: &["minecraft:diamond_sword", "minecraft:diamond"],
            not_found: &["minecraft:stone"],
        },
        SearchRow {
            text: "diamond_sword",
            found: &[],
            not_found: &["minecraft:diamond_sword"],
        },
        SearchRow {
            text: "minecraft:sword",
            found: &["minecraft:diamond_sword", "minecraft:iron_sword"],
            not_found: &["minecraft:diamond"],
        },
        SearchRow {
            text: "random",
            found: &["minecraft:painting"],
            not_found: &["minecraft:stone"],
        },
        SearchRow {
            text: "#swords",
            found: &["minecraft:diamond_sword", "minecraft:iron_sword"],
            not_found: &["minecraft:diamond", "minecraft:stone"],
        },
    ];
    for row in rows {
        let mut state = opened(&VANILLA);
        state.select(SEARCH, &VANILLA);
        state.wheel(-5.0, &VANILLA);
        state.edit_search(&VANILLA, |search| {
            search.insert(row.text);
        });
        let view = state.content(&VANILLA);
        let shown: Vec<String> = (0..view.len())
            .filter_map(|index| view.get(index))
            .map(item_of)
            .collect();
        for item in row.found {
            assert!(
                shown.iter().any(|shown| shown == item),
                "{:?} finds {item}",
                row.text
            );
        }
        for item in row.not_found {
            assert!(
                !shown.iter().any(|shown| shown == item),
                "{:?} does not find {item}",
                row.text
            );
        }
        if !row.text.is_empty() {
            assert_scroll(state.scroll(), 0.0, row.text);
        }
    }
}

#[test]
fn the_search_finds_a_player_head_by_the_name_of_its_player() {
    let id = TABLE
        .id(NameTable::Item, rl!("minecraft:player_head"))
        .expect("the player head is an item");
    let head = ItemStack::new(Kind(id), Count::new(1).expect("a count above 0"));
    let named = head
        .clone()
        .set(Profile::named("Steve").expect("a valid player name"));
    let index = CreativeSearchIndex::build(
        &[named, head],
        &TABLE,
        test_registries(),
        Some(ClientLanguage::corpus()),
    );
    let query = SearchQuery::new("steve");
    assert!(index.matches(0, &query), "the named head is found");
    assert!(!index.matches(1, &query), "a head without a profile is not");
}

#[test]
fn an_empty_tab_is_hidden() {
    let only_building_blocks = sized(10);
    let visible = only_building_blocks.visible_tabs();
    for (tab, shown) in [
        (BUILDING_BLOCKS, true),
        (COLORED_BLOCKS, false),
        (SAVED_HOTBARS, true),
        (SEARCH, true),
        (OP_BLOCKS, false),
        (SURVIVAL_INVENTORY, true),
    ] {
        assert_eq!(visible.contains(tab), shown, "tab {}", tab.index());
    }
}

#[test]
fn the_plugin_shows_the_operator_tab_only_with_the_permission_and_the_option() {
    let mut app = App::new();
    app.add_plugins(CreativeScreenPlugin)
        .insert_resource(CurrentRegistries::new(test_registries().clone()))
        .insert_resource::<ItemTable>(*TABLE)
        .insert_resource(ClientLanguage::load(ResourceSet::corpus()).unwrap())
        .insert_resource(Screen::Menu)
        .init_resource::<ClientOptions>()
        .add_systems(Update, mark_creative_screen.after(CreativeContentSystems));
    let player = app
        .world_mut()
        .spawn((
            Player,
            Abilities::new(GameMode::Creative),
            PermissionLevel::GAMEMASTERS,
        ))
        .id();
    app.world_mut().spawn(MenuOf(player));
    app.update();
    let content = app.world().resource::<CreativeTabContent>();
    assert!(
        !content.visible_tabs().contains(OP_BLOCKS),
        "the option off hides the tab from an operator"
    );
    let search_without_operator = content.search_list().len();

    app.world_mut()
        .resource_mut::<ClientOptions>()
        .operator_items_tab = true;
    app.update();
    let content = app.world().resource::<CreativeTabContent>();
    assert!(
        content.visible_tabs().contains(OP_BLOCKS),
        "an operator in creative with the option on sees the tab"
    );
    assert!(
        content.search_list().len() > search_without_operator,
        "the search list holds the operator stacks only with the condition"
    );
    select(&mut app, OP_BLOCKS);
    assert_eq!(
        app.world().resource::<CreativeScreenState>().selected(),
        OP_BLOCKS
    );

    app.world_mut()
        .entity_mut(player)
        .insert(PermissionLevel(0));
    app.update();
    let content = app.world().resource::<CreativeTabContent>();
    assert!(
        !content.visible_tabs().contains(OP_BLOCKS),
        "a deop hides the tab"
    );
    let screen = app.world().resource::<CreativeScreenState>();
    assert_eq!(
        screen.selected(),
        BUILDING_BLOCKS,
        "the deop moves the selection"
    );

    app.world_mut()
        .entity_mut(player)
        .insert(PermissionLevel::GAMEMASTERS);
    app.update();
    select(&mut app, OP_BLOCKS);
    *app.world_mut().resource_mut::<Screen>() = Screen::None;
    app.update();
    for level in [PermissionLevel(0), PermissionLevel::GAMEMASTERS] {
        app.world_mut().entity_mut(player).insert(level);
        app.update();
    }
    *app.world_mut().resource_mut::<Screen>() = Screen::Menu;
    app.update();
    assert_eq!(
        app.world().resource::<CreativeScreenState>().selected(),
        OP_BLOCKS,
        "a deop and a reop while the screen is closed keep the last tab"
    );
}

fn select(app: &mut App, tab: CreativeTabIndex) {
    app.world_mut()
        .resource_scope(|world, mut screen: Mut<CreativeScreenState>| {
            screen.select(tab, world.resource::<CreativeTabContent>());
        });
}
