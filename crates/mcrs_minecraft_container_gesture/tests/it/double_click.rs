use core::time::Duration;

use mcrs_minecraft_container_click::{Click, ClickKind, Clicked};
use mcrs_minecraft_container_gesture::{GestureButton, GestureClick, MenuGesture, Modifiers};

use crate::test_menu::{EMPTY_HANDED, frame, on_entry};

fn release_after_second_press(
    between: Option<GestureButton>,
    second_entry: u16,
    after: Duration,
) -> Vec<GestureClick> {
    let mut gesture = MenuGesture::default();
    let mut sent = Vec::new();
    let mut frame = frame(EMPTY_HANDED, &mut gesture, &mut sent);
    let left = GestureButton::LEFT;
    let modifiers = Modifiers::default();
    frame.press(left, Duration::ZERO, on_entry(3), modifiers);
    frame.release(left, on_entry(3), modifiers);
    if let Some(button) = between {
        frame.press(button, Duration::ZERO, on_entry(3), modifiers);
        frame.release(button, on_entry(3), modifiers);
    }
    frame.press(left, after, on_entry(second_entry), modifiers);
    frame.sent.clear();
    frame.release(left, on_entry(second_entry), modifiers);
    sent
}

#[test]
fn a_second_left_press_picks_up_all_only_on_the_same_entry_within_250_ms_with_no_press_between() {
    let pick_up_all = |index| {
        GestureClick::Menu(Click {
            kind: ClickKind::PickUpAll,
            button: 0,
            clicked: Clicked::Entry(index),
        })
    };
    let cases = [
        (None, 3, 249, Some(pick_up_all(3))),
        (None, 3, 250, None),
        (None, 4, 100, None),
        (Some(GestureButton::RIGHT), 3, 100, None),
    ];
    for (between, entry, millis, expected) in cases {
        let sent = release_after_second_press(between, entry, Duration::from_millis(millis));
        let picked = sent.iter().copied().find(|click| {
            matches!(
                click,
                GestureClick::Hit(Click {
                    kind: ClickKind::PickUpAll,
                    ..
                }) | GestureClick::Menu(Click {
                    kind: ClickKind::PickUpAll,
                    ..
                })
            )
        });
        assert_eq!(
            picked, expected,
            "{between:?} between, entry {entry} after {millis} ms"
        );
    }
}
