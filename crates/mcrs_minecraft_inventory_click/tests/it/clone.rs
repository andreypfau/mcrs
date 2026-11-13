use mcrs_minecraft_inventory_click::ClickKind;
use mcrs_minecraft_inventory_click::Clicked;
use mcrs_minecraft_inventory_plan::Op;

use crate::fixture::CURSOR;
use crate::fixture::Case;
use crate::fixture::Player;
use crate::fixture::apply;
use crate::fixture::check;
use crate::fixture::click;
use crate::fixture::dirt;
use crate::fixture::stone;

#[test]
fn itx_jav_012_inv_cre_005_clone_sets_the_cursor_to_a_full_copy() {
    let player = Player::new().creative().with(9, stone(3));
    let ops = click(&player, ClickKind::Clone, 7, Clicked::Entry(9)).expect("an entry");
    assert_eq!(
        ops,
        vec![Op::Set {
            slot: CURSOR,
            stack: stone(64),
            count: 64,
        }]
    );
    let after = apply(&player, &ops);
    assert_eq!([after.cursor_stack(), after.at(9)], [stone(64), stone(3)]);
}

#[test]
fn itx_jav_012_clone_conditions() {
    let case = |name, player, clicked| Case {
        name,
        player,
        button: 0,
        clicked,
        ops: vec![],
    };
    check(
        ClickKind::Clone,
        vec![
            case(
                "not creative",
                Player::new().with(9, stone(3)),
                Clicked::Entry(9),
            ),
            case(
                "the cursor holds a stack",
                Player::new().creative().with(9, stone(3)).carrying(dirt(1)),
                Clicked::Entry(9),
            ),
            case("an empty slot", Player::new().creative(), Clicked::Entry(9)),
            case(
                "a negative slot index",
                Player::new().creative().with(9, stone(3)),
                Clicked::Outside,
            ),
        ],
    );
}
