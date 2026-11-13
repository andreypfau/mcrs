use mcrs_minecraft_inventory_click::ClickKind;
use mcrs_minecraft_inventory_click::Clicked;

use crate::fixture::Case;
use crate::fixture::Player;
use crate::fixture::check;
use crate::fixture::dirt;
use crate::fixture::drop_from;
use crate::fixture::entry;
use crate::fixture::locked_helmet;
use crate::fixture::stone;

#[test]
fn itx_jav_013_throw() {
    let case = |name, player, button, clicked, ops| Case {
        name,
        player,
        button,
        clicked,
        ops,
    };
    let held = || Player::new().with(9, stone(10));
    check(
        ClickKind::Throw,
        vec![
            case(
                "button 0 drops one",
                held(),
                0,
                Clicked::Entry(9),
                vec![drop_from(entry(9), 1)],
            ),
            case(
                "button 3 drops all",
                held(),
                3,
                Clicked::Entry(9),
                vec![drop_from(entry(9), 10)],
            ),
            case(
                "the cursor holds a stack",
                held().carrying(dirt(1)),
                0,
                Clicked::Entry(9),
                vec![],
            ),
            case(
                "the take filter refuses",
                Player::new().with(5, locked_helmet()),
                0,
                Clicked::Entry(5),
                vec![],
            ),
            case("an empty slot", Player::new(), 0, Clicked::Entry(9), vec![]),
            case("a negative slot index", held(), 0, Clicked::Outside, vec![]),
        ],
    );
}
