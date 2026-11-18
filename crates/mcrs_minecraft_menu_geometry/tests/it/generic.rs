use mcrs_minecraft_client_widget::GuiPoint;
use mcrs_minecraft_container_click::Clicked;
use mcrs_minecraft_inventory_menu::MenuLayout;
use mcrs_minecraft_menu_geometry::BackgroundPart;
use mcrs_minecraft_menu_geometry::MenuGeometry;
use mcrs_minecraft_menu_geometry::TextureRect;
use mcrs_minecraft_menu_geometry::TitlePosition;
use mcrs_minecraft_menu_key::menu_keys::MenuType;
use mcrs_minecraft_resource::rl;

struct Case {
    name: &'static str,
    kind: Option<MenuType>,
    height: i32,
    entries: u16,
    corners: [(u16, GuiPoint); 5],
}

const fn point(x: i32, y: i32) -> GuiPoint {
    GuiPoint::new(x, y)
}

const CASES: [Case; 9] = [
    Case {
        name: "generic_9x1",
        kind: Some(MenuType::Generic9x1),
        height: 132,
        entries: 45,
        corners: [
            (0, point(8, 18)),
            (8, point(152, 18)),
            (9, point(8, 49)),
            (36, point(8, 107)),
            (44, point(152, 107)),
        ],
    },
    Case {
        name: "generic_9x2",
        kind: Some(MenuType::Generic9x2),
        height: 150,
        entries: 54,
        corners: [
            (0, point(8, 18)),
            (17, point(152, 36)),
            (18, point(8, 67)),
            (45, point(8, 125)),
            (53, point(152, 125)),
        ],
    },
    Case {
        name: "generic_9x3",
        kind: Some(MenuType::Generic9x3),
        height: 168,
        entries: 63,
        corners: [
            (0, point(8, 18)),
            (26, point(152, 54)),
            (27, point(8, 85)),
            (54, point(8, 143)),
            (62, point(152, 143)),
        ],
    },
    Case {
        name: "generic_9x4",
        kind: Some(MenuType::Generic9x4),
        height: 186,
        entries: 72,
        corners: [
            (0, point(8, 18)),
            (35, point(152, 72)),
            (36, point(8, 103)),
            (63, point(8, 161)),
            (71, point(152, 161)),
        ],
    },
    Case {
        name: "generic_9x5",
        kind: Some(MenuType::Generic9x5),
        height: 204,
        entries: 81,
        corners: [
            (0, point(8, 18)),
            (44, point(152, 90)),
            (45, point(8, 121)),
            (72, point(8, 179)),
            (80, point(152, 179)),
        ],
    },
    Case {
        name: "generic_9x6",
        kind: Some(MenuType::Generic9x6),
        height: 222,
        entries: 90,
        corners: [
            (0, point(8, 18)),
            (53, point(152, 108)),
            (54, point(8, 139)),
            (81, point(8, 197)),
            (89, point(152, 197)),
        ],
    },
    Case {
        name: "generic_3x3",
        kind: Some(MenuType::Generic3x3),
        height: 166,
        entries: 45,
        corners: [
            (0, point(62, 17)),
            (8, point(98, 53)),
            (9, point(8, 84)),
            (36, point(8, 142)),
            (44, point(152, 142)),
        ],
    },
    Case {
        name: "hopper",
        kind: Some(MenuType::Hopper),
        height: 133,
        entries: 41,
        corners: [
            (0, point(44, 20)),
            (4, point(116, 20)),
            (5, point(8, 51)),
            (32, point(8, 109)),
            (40, point(152, 109)),
        ],
    },
    Case {
        name: "inventory",
        kind: None,
        height: 166,
        entries: 46,
        corners: [
            (0, point(154, 28)),
            (8, point(8, 62)),
            (9, point(8, 84)),
            (36, point(8, 142)),
            (44, point(152, 142)),
        ],
    },
];

#[test]
fn the_window_and_the_entry_corners_follow_the_menu_type() {
    for case in CASES {
        let name = case.name;
        let geometry = match case.kind {
            Some(kind) => MenuGeometry::of(kind).unwrap(),
            None => MenuGeometry::INVENTORY,
        };
        assert_eq!(
            (geometry.width(), geometry.height()),
            (176, case.height),
            "{name}"
        );
        for (index, corner) in case.corners {
            assert_eq!(geometry.entry(index), Some(corner), "{name}: M[{index}]");
        }
        assert_eq!(geometry.entry(case.entries), None, "{name}");
        assert_eq!(
            geometry.entries().count(),
            usize::from(case.entries),
            "{name}"
        );
        assert_eq!(geometry.is_inventory(), name == "inventory", "{name}");
        if let Some(kind) = case.kind {
            assert_eq!(
                usize::from(MenuLayout::of(kind).unwrap().len()),
                geometry.entries().count(),
                "{name}"
            );
        }
    }
}

#[test]
fn the_background_and_the_labels_follow_the_menu_type() {
    let dispenser = MenuGeometry::of(MenuType::Generic3x3).unwrap();
    assert_eq!(
        dispenser.background().collect::<Vec<_>>(),
        [BackgroundPart::new(
            rl!("minecraft:textures/gui/container/dispenser.png"),
            TextureRect::new(0, 0, 176, 166),
            point(0, 0),
        )]
    );
    let labels = dispenser.labels().unwrap();
    assert_eq!(labels.title, TitlePosition::Centred { y: 6 });
    assert_eq!(labels.player_inventory, point(8, 72));
}

#[test]
fn the_uncovered_rows_of_a_chest_are_in_the_window_but_on_no_entry() {
    let geometry = MenuGeometry::of(MenuType::Generic9x3).unwrap();
    let window = geometry.window(427, 240);
    let cases = [
        (8, 71, Clicked::Nowhere),
        (8, 167, Clicked::Nowhere),
        (8, 168, Clicked::Outside),
    ];
    for (x, y, clicked) in cases {
        let pointer = point(window.x + x, window.y + y);
        assert_eq!(
            geometry.clicked_at(window, pointer),
            clicked,
            "(left + {x}, top + {y})"
        );
    }
}
