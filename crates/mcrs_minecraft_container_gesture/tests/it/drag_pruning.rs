use core::time::Duration;

use mcrs_minecraft_container_click::{Click, ClickKind, Clicked, StackPresence};
use mcrs_minecraft_container_gesture::{
    DragPreview, GestureButton, GestureClick, MenuGesture, Modifiers, PointerMotion,
};

use crate::test_menu::{EMPTY_HANDED, TestMenu, frame, on_entry};

fn dragged_entries(joined: &[u16], left: &'static [u16]) -> Vec<u16> {
    let holding = TestMenu {
        carried: StackPresence::Stack,
        ..EMPTY_HANDED
    };
    let mut gesture = MenuGesture::default();
    let mut sent = Vec::new();
    let mut preview = DragPreview::default();
    let right = GestureButton::RIGHT;
    let modifiers = Modifiers::default();
    let first = joined.first().copied().unwrap_or(0);
    frame(holding, &mut gesture, &mut sent).press(
        right,
        Duration::ZERO,
        on_entry(first),
        modifiers,
    );
    for &index in joined {
        let mut frame = frame(holding, &mut gesture, &mut sent);
        frame.follow_pointer(on_entry(index), PointerMotion::Moved, &mut preview);
    }
    let vanished = TestMenu { left, ..holding };
    let mut frame = frame(vanished, &mut gesture, &mut sent);
    frame.follow_pointer(on_entry(first), PointerMotion::Still, &mut preview);
    frame.sent.clear();
    frame.release(right, on_entry(first), modifiers);
    sent.iter()
        .filter_map(|click| match click {
            GestureClick::Menu(Click {
                kind: ClickKind::Drag,
                clicked: Clicked::Entry(index),
                ..
            }) => Some(*index),
            GestureClick::Hit(_) | GestureClick::Menu(_) => None,
        })
        .collect()
}

#[test]
fn an_entry_that_no_longer_takes_the_drag_leaves_it_only_when_the_drag_holds_two_or_more() {
    let cases: [(&[u16], &'static [u16], &[u16]); 3] = [
        (&[3], &[3], &[3]),
        (&[3, 4], &[4], &[3]),
        (&[3, 4, 5], &[4], &[3, 5]),
    ];
    for (joined, left, expected) in cases {
        assert_eq!(
            dragged_entries(joined, left),
            expected,
            "joined {joined:?}, left {left:?}"
        );
    }
}
