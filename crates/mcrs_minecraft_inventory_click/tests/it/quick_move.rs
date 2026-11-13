use mcrs_minecraft_inventory_click::ClickKind;
use mcrs_minecraft_inventory_click::Clicked;

use crate::fixture::Case;
use crate::fixture::Player;
use crate::fixture::apply;
use crate::fixture::check;
use crate::fixture::click;
use crate::fixture::entry;
use crate::fixture::helmet;
use crate::fixture::locked_helmet;
use crate::fixture::pumpkin;
use crate::fixture::split;
use crate::fixture::stone;
use crate::fixture::sword;

fn case(
    name: &'static str,
    player: Player,
    source: u16,
    ops: Vec<crate::fixture::Operation>,
) -> Case {
    Case {
        name,
        player,
        button: 0,
        clicked: Clicked::Entry(source),
        ops,
    }
}

#[test]
fn itx_jav_009_quick_move_joins_then_fills_the_first_empty_destination() {
    let player = Player::new()
        .with(9, stone(40))
        .with(36, stone(50))
        .with(37, stone(60))
        .with(39, stone(64));
    let ops = click(&player, ClickKind::QuickMove, 1, Clicked::Entry(9)).expect("an entry");
    assert_eq!(
        ops,
        vec![
            split(entry(9), entry(36), 14),
            split(entry(9), entry(37), 4),
            split(entry(9), entry(38), 22),
        ]
    );
    let after = apply(&player, &ops);
    assert_eq!(
        [after.at(9), after.at(36), after.at(37), after.at(38)],
        [None, stone(64), stone(64), stone(22)]
    );
}

#[test]
fn itx_jav_009_inv_mnu_004_quick_move_cases() {
    check(
        ClickKind::QuickMove,
        vec![
            case(
                "a stack that cannot stack skips step 1",
                Player::new().with(9, sword()).with(36, sword()),
                9,
                vec![split(entry(9), entry(37), 1)],
            ),
            case(
                "row 3: armour goes to its empty armour slot",
                Player::new().with(9, helmet()),
                9,
                vec![split(entry(9), entry(5), 1)],
            ),
            case(
                "row 3: the one armour destination takes up to its capacity",
                Player::new().with(9, pumpkin(0, 10)),
                9,
                vec![split(entry(9), entry(5), 1)],
            ),
            case(
                "row 1: the craft result goes to the last entry first",
                Player::new().with(0, stone(10)),
                0,
                vec![split(entry(0), entry(44), 10)],
            ),
            case(
                "the take filter refuses",
                Player::new().with(5, locked_helmet()),
                5,
                vec![],
            ),
            case("an empty source", Player::new(), 9, vec![]),
            Case {
                button: 2,
                ..case(
                    "a button that the table does not list",
                    Player::new().with(9, stone(1)),
                    9,
                    vec![],
                )
            },
        ],
    );
}
