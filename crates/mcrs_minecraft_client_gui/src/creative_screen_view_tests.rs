use std::sync::Arc;

use bevy::math::{IRect, Rect};
use bevy::prelude::*;
use bevy::time::Real;
use mcrs_minecraft_client_world::player::Player;
use mcrs_minecraft_client_world::screen::Screen;
use mcrs_minecraft_engine_item::{ItemCommandsExt, ItemPlugin, StackCommandsExt};
use mcrs_minecraft_game_mode::{Abilities, GameMode};
use mcrs_minecraft_inventory_click::{DragEntry, DragKind, DragSplit};
use mcrs_minecraft_inventory_file::PlayerItems;
use mcrs_minecraft_item_icon::{IconSources, Overlays, StackOverlay, resolve};
use mcrs_minecraft_item_stack::{Count, ItemStack, Kind, NameLookup, NameTable};
use mcrs_minecraft_item_table::ItemTable;
use mcrs_minecraft_menu_geometry::{CreativeTabIndex, CreativeTabRow};
use mcrs_minecraft_model::item_model_asset::DisplayContext;
use mcrs_minecraft_model::item_model_context::ItemModelContext;
use mcrs_minecraft_model::resource_set::ResourceSet;
use mcrs_minecraft_model::stack_view::StackView;
use mcrs_minecraft_player_holders::{PlayerHolders, PlayerHoldersPlugin, restore_holders};
use mcrs_minecraft_resource::ResourceLocation;

use crate::creative_screen_draw::{
    CreativeTextTooltip, draw_creative_screen, rebuild_creative_text,
};
use crate::creative_screen_open::open_creative_screen;
use crate::creative_screen_shown::creative_screen_shown;
use crate::creative_screen_state::CreativeScreenState;
use crate::creative_screen_tests::VANILLA;
use crate::creative_tab_content::CreativeTabContent;
use crate::drag_preview::DragPreview;
use crate::item_decorations::WHITE;
use crate::language::ClientLanguage;
use crate::mark_creative_screen::mark_creative_screen;
use crate::menu_test_app::{ATLAS, MODELS, TABLE};
use crate::scene::{GuiAtlas, GuiQuad, GuiScene, GuiSize, SceneIcon};
use crate::slot_icon::SlotIconPlugin;

const BUILDING_BLOCKS: usize = 0;
const SEARCH: usize = 6;
const OP_BLOCKS: usize = 12;
const SURVIVAL_INVENTORY: usize = 13;
const WINDOW: IVec2 = IVec2::new((427 - 195) / 2, (240 - 136) / 2);

struct TabRow {
    row: CreativeTabRow,
    column: i32,
    x: i32,
    icon: &'static str,
}

const TABS: [TabRow; 14] = [
    tab(CreativeTabRow::Top, 0, 0, "minecraft:bricks"),
    tab(CreativeTabRow::Top, 1, 27, "minecraft:cyan_wool"),
    tab(CreativeTabRow::Top, 2, 54, "minecraft:grass_block"),
    tab(CreativeTabRow::Top, 3, 81, "minecraft:oak_sign"),
    tab(CreativeTabRow::Top, 4, 108, "minecraft:redstone"),
    tab(CreativeTabRow::Top, 5, 142, "minecraft:bookshelf"),
    tab(CreativeTabRow::Top, 6, 169, "minecraft:compass"),
    tab(CreativeTabRow::Bottom, 0, 0, "minecraft:diamond_pickaxe"),
    tab(CreativeTabRow::Bottom, 1, 27, "minecraft:netherite_sword"),
    tab(CreativeTabRow::Bottom, 2, 54, "minecraft:golden_apple"),
    tab(CreativeTabRow::Bottom, 3, 81, "minecraft:iron_ingot"),
    tab(
        CreativeTabRow::Bottom,
        4,
        108,
        "minecraft:creeper_spawn_egg",
    ),
    tab(CreativeTabRow::Bottom, 5, 142, "minecraft:command_block"),
    tab(CreativeTabRow::Bottom, 6, 169, "minecraft:chest"),
];

const fn tab(row: CreativeTabRow, column: i32, x: i32, icon: &'static str) -> TabRow {
    TabRow {
        row,
        column,
        x,
        icon,
    }
}

impl TabRow {
    fn top(&self) -> bool {
        self.row == CreativeTabRow::Top
    }

    fn row_name(&self) -> &'static str {
        match self.row {
            CreativeTabRow::Top => "top",
            CreativeTabRow::Bottom => "bottom",
        }
    }

    fn sprite(&self, state: &str) -> (String, IRect) {
        let y = if self.top() { -28 } else { 132 };
        let min = WINDOW + IVec2::new(self.x, y);
        (
            format!(
                "container/creative_inventory/tab_{}_{state}_{}",
                self.row_name(),
                self.column + 1
            ),
            IRect::from_corners(min, min + IVec2::new(26, 32)),
        )
    }

    fn icon_origin(&self) -> IVec2 {
        let y = if self.top() { -19 } else { 139 };
        WINDOW + IVec2::new(self.x + 5, y)
    }
}

pub(crate) fn item(name: &str) -> Kind {
    let location = ResourceLocation::read(name).unwrap();
    Kind(TABLE.id(NameTable::Item, location).unwrap())
}

fn tab_icon(name: &str) -> SceneIcon {
    let stack = ItemStack::new(item(name), Count::new(1).unwrap());
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

pub(crate) fn creative_app() -> App {
    let mut app = App::new();
    app.insert_resource::<ItemTable>(*TABLE)
        .add_plugins((ItemPlugin::default(), PlayerHoldersPlugin, SlotIconPlugin))
        .insert_resource(MODELS.clone())
        .insert_resource(Screen::None)
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
        .insert_resource(VANILLA.clone())
        .init_resource::<CreativeScreenState>()
        .init_resource::<Time<Real>>()
        .init_resource::<CreativeTextTooltip>()
        .add_observer(open_creative_screen)
        .add_systems(
            Update,
            (
                mark_creative_screen,
                rebuild_creative_text.run_if(resource_exists_and_changed::<ClientLanguage>),
                draw_creative_screen.run_if(creative_screen_shown),
            )
                .chain(),
        );
    let world = app.world_mut();
    let player = world.spawn_empty().id();
    world
        .run_system_cached_with(restore_holders, (player, PlayerItems::default()))
        .unwrap();
    world.flush();
    world
        .entity_mut(player)
        .insert((Player, Abilities::new(GameMode::Creative)));
    for slot in [39, 9, 0, 40] {
        let stack = ItemStack::new(item("minecraft:stone"), Count::new(1).unwrap());
        world.commands().spawn_stack(stack).put_at(player, slot);
        world.flush();
    }
    app
}

fn draw(app: &mut App) {
    let mut scene = app.world_mut().resource_mut::<GuiScene>();
    scene.quads.clear();
    scene.icons.clear();
    app.update();
}

pub(crate) fn creative(
    app: &mut App,
    change: impl FnOnce(&mut CreativeScreenState, &CreativeTabContent),
) {
    let world = app.world_mut();
    world.resource_scope(|world, mut screen: Mut<CreativeScreenState>| {
        change(&mut screen, world.resource());
    });
}

struct DrawRow {
    name: &'static str,
    tab: usize,
    background: &'static str,
    thumb: Option<&'static str>,
    grid: bool,
    items: &'static [(i32, i32)],
}

fn expected_sprites(row: &DrawRow, others: &[usize]) -> Vec<(String, IRect)> {
    let mut expected: Vec<(String, IRect)> = others
        .iter()
        .map(|index| TABS[*index].sprite("unselected"))
        .collect();
    if let Some(thumb) = row.thumb {
        let min = WINDOW + IVec2::new(175, 18);
        expected.push((
            thumb.to_owned(),
            IRect::from_corners(min, min + IVec2::new(12, 15)),
        ));
    }
    expected.push(TABS[row.tab].sprite("selected"));
    expected
}

fn creative_sprites(scene: &GuiScene) -> Vec<(String, IRect)> {
    scene
        .quads
        .iter()
        .filter_map(|quad| match quad {
            GuiQuad::Sprite { region, rect, .. }
                if region.starts_with("container/creative_inventory/") =>
            {
                Some(((*region).to_owned(), *rect))
            }
            _ => None,
        })
        .collect()
}

fn icon_origins(scene: &GuiScene) -> Vec<IVec2> {
    scene
        .quads
        .iter()
        .filter_map(|quad| match quad {
            GuiQuad::Icon { origin, .. } => Some(*origin),
            _ => None,
        })
        .collect()
}

#[test]
fn the_creative_screen_draws_the_tabs_the_background_and_the_slots_of_the_selected_tab() {
    let rows = [
        DrawRow {
            name: "a full items tab",
            tab: BUILDING_BLOCKS,
            background: "container/creative_inventory/tab_items",
            thumb: Some("container/creative_inventory/scroller"),
            grid: true,
            items: &[(9, 112)],
        },
        DrawRow {
            name: "the survival inventory tab",
            tab: SURVIVAL_INVENTORY,
            background: "container/creative_inventory/tab_inventory",
            thumb: None,
            grid: false,
            items: &[(54, 6), (9, 54), (9, 112), (35, 20)],
        },
    ];
    for row in rows {
        let mut app = creative_app();
        app.world_mut()
            .resource_mut::<Screen>()
            .set_if_neq(Screen::Menu);
        draw(&mut app);
        creative(&mut app, |screen, content| {
            screen.select(
                CreativeTabIndex::new(row.tab).expect("a creative tab"),
                content,
            );
        });
        draw(&mut app);
        let scene = app.world().resource::<GuiScene>();

        let image = ATLAS.region(row.background).min.as_vec2();
        let texels: Vec<_> = scene
            .quads
            .iter()
            .filter(|quad| matches!(quad, GuiQuad::Texels { .. }))
            .copied()
            .collect();
        assert_eq!(
            texels,
            [GuiQuad::Texels {
                rect: IRect::from_corners(WINDOW, WINDOW + IVec2::new(195, 136)),
                texels: Rect::from_corners(image, image + Vec2::new(195.0, 136.0)),
                color: WHITE,
            }],
            "{}: the background",
            row.name
        );

        let visible = |index: &usize| *index != OP_BLOCKS;
        let others: Vec<usize> = (0..TABS.len())
            .filter(visible)
            .filter(|index| *index != row.tab)
            .collect();
        assert_eq!(
            creative_sprites(scene),
            expected_sprites(&row, &others),
            "{}: the tab and thumb sprites",
            row.name
        );

        let mut origins: Vec<IVec2> = [row.tab]
            .iter()
            .chain(&others)
            .map(|index| TABS[*index].icon_origin())
            .collect();
        let tab_icons: Vec<SceneIcon> = [row.tab]
            .iter()
            .chain(&others)
            .map(|index| tab_icon(TABS[*index].icon))
            .collect();
        if row.grid {
            origins.extend(
                (0..45).map(|slot| WINDOW + IVec2::new(9 + 18 * (slot % 9), 18 + 18 * (slot / 9))),
            );
        }
        origins.extend(row.items.iter().map(|(x, y)| WINDOW + IVec2::new(*x, *y)));
        assert_eq!(
            icon_origins(scene),
            origins,
            "{}: the icon positions",
            row.name
        );
        assert_eq!(
            scene.icons.get(..tab_icons.len()),
            Some(tab_icons.as_slice()),
            "{}: the tab icons are their stacks",
            row.name
        );
        assert!(
            scene.icons[tab_icons.len()..]
                .iter()
                .all(|icon| icon.overlays == Overlays::Stack),
            "{}: the grid stacks and the menu entries draw with their decorations",
            row.name
        );
    }
}

#[test]
fn the_open_clears_the_search_and_the_scroll_and_keeps_the_tab() {
    let mut app = creative_app();
    let type_and_scroll = |screen: &mut CreativeScreenState, content: &CreativeTabContent| {
        screen.edit_search(content, |search| {
            search.insert("st");
        });
        screen.wheel(-1.0, content);
    };
    creative(&mut app, |screen, content| {
        screen.select(
            CreativeTabIndex::new(SEARCH).expect("a creative tab"),
            content,
        );
        type_and_scroll(screen, content);
    });
    draw(&mut app);
    let state = &app.world().resource::<CreativeScreenState>();
    assert_eq!(state.search_box().text(), "st", "no open while hidden");
    assert!(state.scroll() > 0.0, "the search result scrolls");

    app.world_mut()
        .resource_mut::<Screen>()
        .set_if_neq(Screen::Menu);
    draw(&mut app);
    let state = &app.world().resource::<CreativeScreenState>();
    assert_eq!(state.selected().index(), SEARCH, "the open keeps the tab");
    assert_eq!(state.search_box().text(), "", "the open clears the search");
    assert_eq!(state.scroll(), 0.0, "the open resets the scroll");

    creative(&mut app, type_and_scroll);
    draw(&mut app);
    let state = &app.world().resource::<CreativeScreenState>();
    assert_eq!(
        state.search_box().text(),
        "st",
        "a frame of a shown screen is not an open"
    );

    app.world_mut()
        .resource_mut::<Screen>()
        .set_if_neq(Screen::None);
    draw(&mut app);
    app.world_mut()
        .resource_mut::<Screen>()
        .set_if_neq(Screen::Menu);
    draw(&mut app);
    let state = &app.world().resource::<CreativeScreenState>();
    assert_eq!(state.search_box().text(), "", "each open clears the search");
}

#[test]
fn a_steady_frame_over_a_tab_draws_its_title_without_allocation() {
    let mut app = creative_app();
    app.world_mut()
        .resource_mut::<Screen>()
        .set_if_neq(Screen::Menu);
    draw(&mut app);
    let (_, colored_blocks) = TABS[1].sprite("unselected");
    let world = app.world_mut();
    world
        .resource_mut::<GuiScene>()
        .map_unchanged(|resource| &mut resource.cursor)
        .set_if_neq(colored_blocks.center());
    let system = world.register_system(draw_creative_screen);
    let frame = |world: &mut World| {
        let mut scene = world.resource_mut::<GuiScene>();
        scene.quads.clear();
        scene.icons.clear();
        scene.tooltip.clear();
        world.run_system(system).unwrap();
    };
    frame(world);
    frame(world);
    let before = mcrs_test_alloc::allocations();
    frame(world);
    let allocations = mcrs_test_alloc::allocations() - before;
    assert_eq!(allocations, 0, "the pointer stays over the same tab");
    let scene = app.world().resource::<GuiScene>();
    let lines = scene
        .tooltip
        .lines()
        .expect("the hovered tab shows its title");
    let title: Vec<&str> = lines
        .iter()
        .flatten()
        .map(|span| span.text.as_str())
        .collect();
    assert_eq!(title, ["Colored Blocks"]);
}

#[test]
fn a_drag_over_the_hotbar_previews_the_split_and_the_rest_of_the_carried_stack() {
    let mut app = creative_app();
    let world = app.world_mut();
    let player = world
        .query_filtered::<&PlayerHolders, With<Player>>()
        .single(world)
        .unwrap()
        .cursor;
    let stack = ItemStack::new(item("minecraft:stone"), Count::new(10).unwrap());
    world.commands().spawn_stack(stack).put_at(player, 0);
    world.flush();
    let mut drag = DragPreview::default();
    for index in [37, 38, 39] {
        drag.join(index, DragEntry { held: 0, cap: 64 });
    }
    drag.split(DragSplit {
        kind: DragKind::Split,
        count: 10,
        max: 64,
        size: 3,
    });
    world.insert_resource(drag);
    world.resource_mut::<Screen>().set_if_neq(Screen::Menu);
    let cursor = IVec2::new(400, 230);
    world
        .resource_mut::<GuiScene>()
        .map_unchanged(|resource| &mut resource.cursor)
        .set_if_neq(cursor);
    draw(&mut app);
    let scene = app.world().resource::<GuiScene>();
    let corners: Vec<IVec2> = (1..4)
        .map(|column| WINDOW + IVec2::new(9 + 18 * column, 112))
        .collect();
    let fills: Vec<IRect> = (scene.quads.iter())
        .filter_map(|quad| match quad {
            GuiQuad::Fill {
                rect,
                color: 0x80FF_FFFF,
            } => Some(*rect),
            _ => None,
        })
        .collect();
    let areas: Vec<IRect> = (corners.iter())
        .map(|corner| IRect::from_corners(*corner, *corner + 16))
        .collect();
    assert_eq!(fills, areas, "a white fill on each previewed hotbar entry");
    let drawn: Vec<(IVec2, u8)> = (scene.quads.iter())
        .filter_map(|quad| match *quad {
            GuiQuad::Icon { origin, index }
                if corners.contains(&origin) || origin == cursor - 8 =>
            {
                Some((origin, scene.icons[index].overlay.count))
            }
            _ => None,
        })
        .collect();
    assert_eq!(
        drawn,
        vec![
            (corners[0], 3),
            (corners[1], 3),
            (corners[2], 3),
            (cursor - 8, 1)
        ],
        "3 in each previewed entry and 1 at the mouse"
    );
}
