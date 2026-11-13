use mcrs_minecraft_inventory_click::ClickError;
use mcrs_minecraft_inventory_click::ClickKind;
use mcrs_minecraft_inventory_click::Clicked;

use crate::fixture::CURSOR;
use crate::fixture::Case;
use crate::fixture::Player;
use crate::fixture::check;
use crate::fixture::click;
use crate::fixture::dirt;
use crate::fixture::drop_from;
use crate::fixture::entry;
use crate::fixture::helmet;
use crate::fixture::locked_helmet;
use crate::fixture::pumpkin;
use crate::fixture::split;
use crate::fixture::stone;
use crate::fixture::swap;

const MAIN: u16 = 9;
const HEAD: u16 = 5;
const RESULT: u16 = 0;

fn case(
    name: &'static str,
    player: Player,
    button: u8,
    ops: Vec<crate::fixture::Operation>,
) -> Case {
    Case {
        name,
        player,
        button,
        clicked: Clicked::Entry(MAIN),
        ops,
    }
}

#[test]
#[expect(clippy::too_many_lines, reason = "one table of cases")]
fn itx_jav_008_pick_up_rows() {
    let s = entry(MAIN);
    let head = entry(HEAD);
    let result = entry(RESULT);
    let at_head = |name, player: Player, ops| Case {
        clicked: Clicked::Entry(HEAD),
        ..case(name, player, 0, ops)
    };
    let at_result = |name, player: Player, ops| Case {
        clicked: Clicked::Entry(RESULT),
        ..case(name, player, 0, ops)
    };
    check(
        ClickKind::PickUp,
        vec![
            case("row 2: both empty", Player::new(), 0, vec![]),
            at_head(
                "row 3: the place filter refuses the cursor stack",
                Player::new().carrying(stone(10)),
                vec![],
            ),
            case(
                "row 4: place all",
                Player::new().carrying(stone(10)),
                0,
                vec![split(CURSOR, s, 10)],
            ),
            case(
                "row 4: place one",
                Player::new().carrying(stone(10)),
                1,
                vec![split(CURSOR, s, 1)],
            ),
            at_head(
                "row 4: the capacity limits the place",
                Player::new().carrying(pumpkin(0, 3)),
                vec![split(CURSOR, head, 1)],
            ),
            at_head(
                "row 5: the take filter refuses",
                Player::new().with(HEAD, locked_helmet()),
                vec![],
            ),
            case(
                "row 6: take all",
                Player::new().with(MAIN, stone(10)),
                0,
                vec![swap(s, CURSOR)],
            ),
            case(
                "row 7: take half, rounded up",
                Player::new().with(MAIN, stone(7)),
                1,
                vec![split(s, CURSOR, 4)],
            ),
            case(
                "row 8: join up to the capacity",
                Player::new().with(MAIN, stone(60)).carrying(stone(10)),
                0,
                vec![split(CURSOR, s, 4)],
            ),
            case(
                "row 8: join one",
                Player::new().with(MAIN, stone(60)).carrying(stone(10)),
                1,
                vec![split(CURSOR, s, 1)],
            ),
            case(
                "row 8: a full slot gives no operation",
                Player::new().with(MAIN, stone(64)).carrying(stone(10)),
                0,
                vec![],
            ),
            case(
                "row 9: exchange different stacks",
                Player::new().with(MAIN, stone(10)).carrying(dirt(5)),
                0,
                vec![swap(s, CURSOR)],
            ),
            at_head(
                "row 10: the cursor stack is larger than the capacity",
                Player::new().with(HEAD, helmet()).carrying(pumpkin(0, 3)),
                vec![],
            ),
            at_result(
                "row 11: take all that the cursor can hold",
                Player::new().with(RESULT, stone(10)).carrying(stone(50)),
                vec![split(result, CURSOR, 10)],
            ),
            at_result(
                "row 11: a part when place(S, S) is false gives no operation",
                Player::new().with(RESULT, stone(10)).carrying(stone(60)),
                vec![],
            ),
            case(
                "a button that the table does not list",
                Player::new().with(MAIN, stone(10)),
                2,
                vec![],
            ),
        ],
    );
}

#[test]
fn itx_jav_007_outside_the_window_drops_from_the_cursor() {
    let outside = |name, player: Player, button, clicked, ops| Case {
        clicked,
        ..case(name, player, button, ops)
    };
    for kind in [ClickKind::PickUp, ClickKind::QuickMove] {
        check(
            kind,
            vec![
                outside(
                    "button 0 drops all",
                    Player::new().carrying(stone(10)),
                    0,
                    Clicked::Outside,
                    vec![drop_from(CURSOR, 10)],
                ),
                outside(
                    "button 1 drops one",
                    Player::new().carrying(stone(10)),
                    1,
                    Clicked::Outside,
                    vec![drop_from(CURSOR, 1)],
                ),
                outside(
                    "an empty cursor",
                    Player::new(),
                    0,
                    Clicked::Outside,
                    vec![],
                ),
                outside(
                    "another negative slot index",
                    Player::new().carrying(stone(10)),
                    0,
                    Clicked::from(-1),
                    vec![],
                ),
            ],
        );
    }
}

#[test]
fn itx_jav_001_slot_index_maps_and_an_index_past_the_menu_is_ignored() {
    assert_eq!(Clicked::from(-999), Clicked::Outside);
    assert_eq!(Clicked::from(-5), Clicked::Nowhere);
    assert_eq!(Clicked::from(45), Clicked::Entry(45));
    let player = Player::new().carrying(stone(10));
    assert_eq!(
        click(&player, ClickKind::PickUp, 0, Clicked::Entry(46)),
        Err(ClickError::NoSuchEntry)
    );
}
