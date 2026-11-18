use std::time::Duration;

use bevy::prelude::*;
use bevy::time::Real;
use mcrs_minecraft_client_world::player::Player;
use mcrs_minecraft_client_world::screen::Screen;
use mcrs_minecraft_engine_item::{ItemCommandsExt, StackCommandsExt};
use mcrs_minecraft_game_registries::registry_load::test_registries;
use mcrs_minecraft_item_stack::{Count, ItemStack};
use mcrs_minecraft_menu_geometry::{CreativeGeometry, CreativeTabIndex};
use mcrs_minecraft_registries::CurrentRegistries;

use crate::creative_item_tooltip::request_creative_item_tooltip;
use crate::creative_screen_draw::draw_creative_screen;
use crate::creative_screen_shown::creative_screen_shown;
use crate::creative_screen_view_tests::{creative, creative_app, item};
use crate::scene::{GuiConfig, GuiScene};

const BUILDING_BLOCKS: usize = 0;
const SEARCH: usize = 6;
const SURVIVAL_INVENTORY: usize = 13;
const HOTBAR_COLUMN: u16 = 1;
const HOTBAR_INVENTORY_SLOT: u16 = 1;
const NAME: u32 = 0xFFFF_FFFF;
const TAB: u32 = 0xFF55_55FF;
const TAG: u32 = 0xFFAA_00AA;
const RANDOM: u32 = 0xFFAA_AAAA;

#[derive(Clone, Copy, Debug)]
enum Target {
    GridOf(&'static str),
    Hotbar(&'static str),
    Destroy,
}

struct TooltipRow {
    name: &'static str,
    tab: usize,
    search: &'static str,
    target: Target,
    expected: &'static [(&'static str, u32)],
}

fn frame(app: &mut App) {
    let world = app.world_mut();
    world
        .resource_mut::<Time<Real>>()
        .update_with_duration(Duration::from_millis(5));
    world.resource_mut::<GuiScene>().tooltip.clear();
    app.update();
}

fn tooltip_app() -> App {
    let mut app = creative_app();
    app.insert_resource(CurrentRegistries::new(test_registries().clone()))
        .insert_resource(GuiConfig {
            scale: 1,
            cursor: None,
            frozen_ticks: None,
            advanced_tooltips: false,
        })
        .add_systems(
            Update,
            request_creative_item_tooltip
                .after(draw_creative_screen)
                .run_if(creative_screen_shown),
        );
    *app.world_mut().resource_mut::<Screen>() = Screen::Menu;
    frame(&mut app);
    app
}

fn hover_point(app: &mut App, target: Target) -> IVec2 {
    let corner = match target {
        Target::GridOf(name) => {
            let mut found = None;
            creative(app, |screen, content| {
                found = (0..45).find(|slot| {
                    screen
                        .grid_stack(*slot, content)
                        .is_some_and(|stack| stack.kind == item(name))
                });
            });
            let slot = found.unwrap_or_else(|| panic!("the grid shows {name}"));
            CreativeGeometry::grid_slot(slot).unwrap()
        }
        Target::Hotbar(name) => {
            let world = app.world_mut();
            let player = world
                .query_filtered::<Entity, With<Player>>()
                .single(world)
                .unwrap();
            let stack = ItemStack::new(item(name), Count::new(1).unwrap());
            world
                .commands()
                .spawn_stack(stack)
                .put_at(player, HOTBAR_INVENTORY_SLOT);
            world.flush();
            CreativeGeometry::hotbar_slot(HOTBAR_COLUMN).unwrap()
        }
        Target::Destroy => CreativeGeometry::DESTROY_SLOT,
    };
    let window = CreativeGeometry::window(427, 240);
    IVec2::new(window.x + corner.x + 8, window.y + corner.y + 8)
}

fn shown_lines(app: &App) -> Vec<(String, u32)> {
    let scene = app.world().resource::<GuiScene>();
    let mut lines: Vec<(String, u32)> = scene
        .tooltip
        .lines()
        .unwrap_or_default()
        .iter()
        .map(|line| {
            let text = line.iter().map(|span| span.text.as_str()).collect();
            (text, line.first().map_or(0, |span| span.color))
        })
        .collect();
    let tags = lines.iter().position(|line| line.1 == TAG);
    if let Some(start) = tags {
        let end = lines[start..]
            .iter()
            .position(|line| line.1 != TAG)
            .map_or(lines.len(), |len| start + len);
        lines[start..end].sort();
    }
    lines
}

#[test]
fn the_creative_screen_shows_the_item_tooltip_of_the_hovered_slot_with_the_creative_parts() {
    let rows = [
        TooltipRow {
            name: "a grid slot of an items tab adds no tab part",
            tab: BUILDING_BLOCKS,
            search: "",
            target: Target::GridOf("minecraft:oak_log"),
            expected: &[("Oak Log", NAME)],
        },
        TooltipRow {
            name: "a hotbar slot adds the tabs that hold the stack",
            tab: SEARCH,
            search: "",
            target: Target::Hotbar("minecraft:oak_log"),
            expected: &[
                ("Oak Log", NAME),
                ("Building Blocks", TAB),
                ("Natural Blocks", TAB),
            ],
        },
        TooltipRow {
            name: "a grid slot shows a painting with no variant as random",
            tab: SEARCH,
            search: "painting",
            target: Target::GridOf("minecraft:painting"),
            expected: &[
                ("Painting", NAME),
                ("Functional Blocks", TAB),
                ("Random variant", RANDOM),
            ],
        },
        TooltipRow {
            name: "a hotbar slot shows no random part",
            tab: SEARCH,
            search: "",
            target: Target::Hotbar("minecraft:painting"),
            expected: &[("Painting", NAME), ("Functional Blocks", TAB)],
        },
        TooltipRow {
            name: "a tag search adds the matched tags on the search grid",
            tab: SEARCH,
            search: "#logs",
            target: Target::GridOf("minecraft:oak_log"),
            expected: &[
                ("Oak Log", NAME),
                ("Building Blocks", TAB),
                ("Natural Blocks", TAB),
                ("#minecraft:logs", TAG),
                ("#minecraft:logs_that_burn", TAG),
                ("#minecraft:oak_logs", TAG),
            ],
        },
        TooltipRow {
            name: "the destroy slot with an empty cursor",
            tab: SURVIVAL_INVENTORY,
            search: "",
            target: Target::Destroy,
            expected: &[("Destroy Item", NAME)],
        },
    ];
    for row in rows {
        let mut app = tooltip_app();
        creative(&mut app, |screen, content| {
            screen.select(CreativeTabIndex::new(row.tab).unwrap(), content);
            screen.edit_search(content, |search| {
                search.insert(row.search);
            });
        });
        let at = hover_point(&mut app, row.target);
        app.world_mut().resource_mut::<GuiScene>().cursor = at;
        frame(&mut app);
        frame(&mut app);
        let expected: Vec<(String, u32)> = row
            .expected
            .iter()
            .map(|(text, color)| ((*text).to_owned(), *color))
            .collect();
        assert_eq!(shown_lines(&app), expected, "{}", row.name);
    }
}
