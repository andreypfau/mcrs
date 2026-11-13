use mcrs_minecraft_inventory_click::ClickKind;
use mcrs_minecraft_inventory_click::Clicked;

use crate::fixture::CURSOR;
use crate::fixture::Case;
use crate::fixture::Player;
use crate::fixture::apply;
use crate::fixture::check;
use crate::fixture::click;
use crate::fixture::entry;
use crate::fixture::split;
use crate::fixture::stone;

/// The craft result is excluded, a full stack waits for the second visit, and a slot that the
/// first visit empties is not taken again.
fn scattered() -> Player {
    Player::new()
        .carrying(stone(10))
        .with(0, stone(5))
        .with(1, stone(64))
        .with(10, stone(20))
        .with(40, stone(30))
}

#[test]
fn itx_jav_011_inv_mnu_005_pick_up_all_visits_two_times() {
    let player = scattered();
    let ops = click(&player, ClickKind::PickUpAll, 0, Clicked::Entry(9)).expect("an entry");
    assert_eq!(
        ops,
        vec![
            split(entry(10), CURSOR, 20),
            split(entry(40), CURSOR, 30),
            split(entry(1), CURSOR, 4),
        ]
    );
    let after = apply(&player, &ops);
    assert_eq!(
        [
            after.cursor_stack(),
            after.at(0),
            after.at(1),
            after.at(10),
            after.at(40)
        ],
        [stone(64), stone(5), stone(60), None, None]
    );
}

#[test]
fn itx_jav_011_pick_up_all_cases() {
    let case = |name, player, button, clicked, ops| Case {
        name,
        player,
        button,
        clicked,
        ops,
    };
    check(
        ClickKind::PickUpAll,
        vec![
            case(
                "another button visits backward",
                scattered(),
                1,
                Clicked::Entry(9),
                vec![
                    split(entry(40), CURSOR, 30),
                    split(entry(10), CURSOR, 20),
                    split(entry(1), CURSOR, 4),
                ],
            ),
            case(
                "the clicked slot holds a stack that it gives",
                scattered().with(9, stone(1)),
                0,
                Clicked::Entry(9),
                vec![],
            ),
            case(
                "an empty cursor",
                scattered().carrying(None),
                0,
                Clicked::Entry(9),
                vec![],
            ),
            case(
                "a negative slot index",
                scattered(),
                0,
                Clicked::Outside,
                vec![],
            ),
        ],
    );
}
