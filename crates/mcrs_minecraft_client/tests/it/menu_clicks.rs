use mcrs_minecraft_client::test_support::ServerAnswers;
use mcrs_minecraft_container_click::Click;
use mcrs_minecraft_container_click::ClickKind;
use mcrs_minecraft_container_click::Clicked;
use mcrs_minecraft_game_mode::GameMode;
use mcrs_minecraft_inventory_client::MenuAction;
use mcrs_minecraft_inventory_client::MenuInput;

use crate::local_world::{self, LocalWorld};
use crate::stacks::AGREED;
use crate::stacks::Entry;
use crate::stacks::Item;
use crate::stacks::Stack;
use crate::stacks::at;
use crate::stacks::click;
use crate::stacks::empty;
use crate::stacks::held;
use crate::stacks::on;

struct Row {
    name: &'static str,
    game_mode: GameMode,
    given: &'static [Entry],
    local: &'static [Entry],
    clicks: &'static [Click],
    entries: &'static [Entry],
    cursor: Option<Stack>,
    answers: ServerAnswers,
}

const ROWS: &[Row] = &[
    Row {
        name: "pick up",
        game_mode: GameMode::Creative,
        given: &[at(9, Item::Stone, 10)],
        local: &[],
        clicks: &[on(ClickKind::PickUp, 0, 9)],
        entries: &[empty(9)],
        cursor: held(Item::Stone, 10),
        answers: AGREED,
    },
    Row {
        name: "place",
        game_mode: GameMode::Creative,
        given: &[],
        local: &[],
        clicks: &[on(ClickKind::PickUp, 0, 10)],
        entries: &[at(10, Item::Stone, 10)],
        cursor: None,
        answers: AGREED,
    },
    Row {
        name: "split with a right click",
        game_mode: GameMode::Creative,
        given: &[],
        local: &[],
        clicks: &[on(ClickKind::PickUp, 1, 10)],
        entries: &[at(10, Item::Stone, 5)],
        cursor: held(Item::Stone, 5),
        answers: AGREED,
    },
    Row {
        name: "swap with a cursor stack",
        game_mode: GameMode::Creative,
        given: &[at(11, Item::Dirt, 3)],
        local: &[],
        clicks: &[on(ClickKind::PickUp, 0, 11)],
        entries: &[at(11, Item::Stone, 5)],
        cursor: held(Item::Dirt, 3),
        answers: AGREED,
    },
    Row {
        name: "drag of three slots",
        game_mode: GameMode::Creative,
        given: &[],
        local: &[],
        clicks: &[
            click(ClickKind::Drag, 0, Clicked::Outside),
            on(ClickKind::Drag, 1, 12),
            on(ClickKind::Drag, 1, 13),
            on(ClickKind::Drag, 1, 14),
            click(ClickKind::Drag, 2, Clicked::Outside),
        ],
        entries: &[
            at(12, Item::Dirt, 1),
            at(13, Item::Dirt, 1),
            at(14, Item::Dirt, 1),
        ],
        cursor: None,
        answers: AGREED,
    },
    Row {
        name: "quick move",
        game_mode: GameMode::Creative,
        given: &[],
        local: &[],
        clicks: &[on(ClickKind::QuickMove, 0, 10)],
        entries: &[empty(10), at(36, Item::Stone, 5)],
        cursor: None,
        answers: AGREED,
    },
    Row {
        name: "number key",
        game_mode: GameMode::Creative,
        given: &[],
        local: &[],
        clicks: &[on(ClickKind::Swap, 2, 11)],
        entries: &[empty(11), at(38, Item::Stone, 5)],
        cursor: None,
        answers: AGREED,
    },
    Row {
        name: "off-hand key",
        game_mode: GameMode::Creative,
        given: &[],
        local: &[],
        clicks: &[on(ClickKind::Swap, 40, 38)],
        entries: &[empty(38), at(45, Item::Stone, 5)],
        cursor: None,
        answers: AGREED,
    },
    Row {
        name: "clone in creative",
        game_mode: GameMode::Creative,
        given: &[],
        local: &[],
        clicks: &[on(ClickKind::Clone, 2, 45)],
        entries: &[at(45, Item::Stone, 5)],
        cursor: held(Item::Stone, 64),
        answers: AGREED,
    },
    Row {
        name: "outside the window drops the cursor",
        game_mode: GameMode::Creative,
        given: &[],
        local: &[],
        clicks: &[click(ClickKind::PickUp, 0, Clicked::Outside)],
        entries: &[],
        cursor: None,
        answers: AGREED,
    },
    Row {
        name: "throw one",
        game_mode: GameMode::Creative,
        given: &[],
        local: &[],
        clicks: &[on(ClickKind::Throw, 0, 36)],
        entries: &[at(36, Item::Stone, 4)],
        cursor: None,
        answers: AGREED,
    },
    Row {
        name: "throw the stack",
        game_mode: GameMode::Creative,
        given: &[],
        local: &[],
        clicks: &[on(ClickKind::Throw, 1, 36)],
        entries: &[empty(36)],
        cursor: None,
        answers: AGREED,
    },
    Row {
        name: "a wrong prediction is repaired slot by slot",
        game_mode: GameMode::Creative,
        given: &[at(20, Item::Stone, 8)],
        local: &[at(20, Item::Stone, 3)],
        clicks: &[on(ClickKind::PickUp, 1, 20)],
        entries: &[at(20, Item::Stone, 4)],
        cursor: held(Item::Stone, 4),
        answers: ServerAnswers {
            slots: 1,
            cursors: 1,
            ..AGREED
        },
    },
    Row {
        name: "split, quick move and a clone that gives nothing in survival",
        game_mode: GameMode::Survival,
        given: &[at(25, Item::Stone, 6), at(26, Item::Dirt, 2)],
        local: &[],
        clicks: &[
            on(ClickKind::PickUp, 0, 27),
            on(ClickKind::PickUp, 1, 25),
            on(ClickKind::PickUp, 0, 28),
            on(ClickKind::QuickMove, 0, 26),
            on(ClickKind::Clone, 2, 45),
        ],
        entries: &[
            at(25, Item::Stone, 3),
            empty(26),
            at(27, Item::Stone, 4),
            at(28, Item::Stone, 3),
            at(36, Item::Dirt, 2),
        ],
        cursor: None,
        answers: AGREED,
    },
];

#[test]
fn each_click_leaves_the_client_and_the_server_with_the_same_stacks() {
    let mut world = LocalWorld::join("menu-clicks", local_world::discard_thrown_items, |_| {});
    let viewer = world.viewer;
    for row in ROWS {
        world.give(row.given);
        world.put_local(row.local);
        world.switch_game_mode(row.game_mode);
        world.clear_answers();
        for &click in row.clicks {
            world.app.world_mut().write_message(MenuInput {
                viewer,
                action: MenuAction::Click(click),
            });
        }
        world.app.update();
        world.assert_after_round_trip(row.name, Some(row.answers), row.entries, row.cursor);
    }
    world.leave();
}
