use mcrs_minecraft_client_widget::GuiPoint;
use mcrs_minecraft_container_click::Clicked;
use mcrs_minecraft_menu_geometry::MenuGeometry;

const GEOMETRY: MenuGeometry = MenuGeometry::INVENTORY;

#[test]
fn the_window_is_centred_with_floor_division() {
    let cases = [
        (427, 240, GuiPoint::new(125, 37)),
        (176, 166, GuiPoint::new(0, 0)),
        (175, 163, GuiPoint::new(-1, -2)),
    ];
    for (width, height, corner) in cases {
        assert_eq!(GEOMETRY.window(width, height), corner, "{width}x{height}");
    }
}

#[test]
fn the_pointer_index_follows_the_window_and_the_entry_areas() {
    let window = GEOMETRY.window(427, 240);
    let cases = [
        (7, 84, Clicked::Nowhere),
        (8, 84, Clicked::Entry(9)),
        (23, 99, Clicked::Entry(9)),
        (24, 84, Clicked::Nowhere),
        (25, 84, Clicked::Nowhere),
        (26, 84, Clicked::Entry(10)),
        (8, 100, Clicked::Nowhere),
        (154, 28, Clicked::Entry(0)),
        (167, 157, Clicked::Entry(44)),
        (0, 0, Clicked::Nowhere),
        (175, 165, Clicked::Nowhere),
        (176, 165, Clicked::Outside),
        (175, 166, Clicked::Outside),
        (-1, 0, Clicked::Outside),
        (0, -1, Clicked::Outside),
    ];
    for (x, y, clicked) in cases {
        let pointer = GuiPoint::new(window.x + x, window.y + y);
        assert_eq!(
            GEOMETRY.clicked_at(window, pointer),
            clicked,
            "(left + {x}, top + {y})"
        );
    }
}
