use mcrs_minecraft_inventory_click::ClickKind;
use mcrs_minecraft_inventory_click::Clicked;

use crate::fixture::Case;
use crate::fixture::Operation;
use crate::fixture::Player;
use crate::fixture::apply;
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

const HOTBAR_0: u16 = 36;
const OFF_HAND: u16 = 45;
const HEAD: u16 = 5;

fn case(name: &'static str, player: Player, source: u16, ops: Vec<Operation>) -> Case {
    Case {
        name,
        player,
        button: 0,
        clicked: Clicked::Entry(source),
        ops,
    }
}

#[test]
fn itx_jav_010_swap_rows() {
    let (s, h, head) = (entry(9), entry(HOTBAR_0), entry(HEAD));
    check(
        ClickKind::Swap,
        vec![
            Case {
                clicked: Clicked::Nowhere,
                ..case(
                    "row 1: a negative slot index",
                    Player::new().with(HOTBAR_0, stone(1)),
                    9,
                    vec![],
                )
            },
            Case {
                button: 9,
                ..case(
                    "a button that names no slot",
                    Player::new().with(9, stone(1)),
                    9,
                    vec![],
                )
            },
            case("row 2: both empty", Player::new(), 9, vec![]),
            case(
                "row 3: the key slot is empty",
                Player::new().with(9, stone(5)),
                9,
                vec![swap(s, h)],
            ),
            case(
                "row 3: the take filter refuses",
                Player::new().with(HEAD, locked_helmet()),
                HEAD,
                vec![],
            ),
            case(
                "row 4: the place filter refuses",
                Player::new().with(HOTBAR_0, stone(5)),
                HEAD,
                vec![],
            ),
            case(
                "row 5: the key stack moves to the empty slot",
                Player::new().with(HOTBAR_0, stone(5)),
                9,
                vec![swap(h, s)],
            ),
            case(
                "row 6: the empty slot takes its capacity",
                Player::new().with(HOTBAR_0, pumpkin(0, 10)),
                HEAD,
                vec![split(h, head, 1)],
            ),
            case(
                "row 7: the place filter refuses",
                Player::new().with(HEAD, helmet()).with(HOTBAR_0, stone(5)),
                HEAD,
                vec![],
            ),
            case(
                "row 8: exchange",
                Player::new().with(9, stone(5)).with(HOTBAR_0, dirt(7)),
                9,
                vec![swap(s, h)],
            ),
            Case {
                button: 40,
                ..case(
                    "row 3: the off-hand key",
                    Player::new().with(9, stone(5)),
                    9,
                    vec![swap(s, entry(OFF_HAND))],
                )
            },
            case(
                "the key slot is the clicked slot",
                Player::new().with(HOTBAR_0, stone(5)),
                HOTBAR_0,
                vec![],
            ),
        ],
    );
}

#[test]
fn itx_jav_010_row_9_inserts_the_clicked_stack_then_fills_it() {
    let player = Player::new()
        .with(HEAD, pumpkin(1, 1))
        .with(HOTBAR_0, pumpkin(0, 10))
        .with(41, pumpkin(1, 63));
    let ops = click(&player, ClickKind::Swap, 0, Clicked::Entry(HEAD)).expect("an entry");
    assert_eq!(
        ops,
        vec![
            split(entry(HEAD), entry(41), 1),
            split(entry(HOTBAR_0), entry(HEAD), 1),
        ]
    );
    let after = apply(&player, &ops);
    assert_eq!(
        [after.at(HEAD), after.at(HOTBAR_0), after.at(41)],
        [pumpkin(0, 1), pumpkin(0, 9), pumpkin(1, 64)]
    );
}

#[test]
fn itx_jav_010_row_9_drops_what_finds_no_room() {
    let mut player = Player::new()
        .with(HEAD, pumpkin(1, 1))
        .with(HOTBAR_0, pumpkin(0, 10));
    for index in (9..45).filter(|index| *index != HOTBAR_0) {
        player = player.with(index, stone(64));
    }
    let ops = click(&player, ClickKind::Swap, 0, Clicked::Entry(HEAD)).expect("an entry");
    assert_eq!(
        ops,
        vec![
            drop_from(entry(HEAD), 1),
            split(entry(HOTBAR_0), entry(HEAD), 1),
        ]
    );
}
