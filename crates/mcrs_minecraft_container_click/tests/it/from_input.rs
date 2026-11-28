use mcrs_minecraft_container_click::ButtonBinding;
use mcrs_minecraft_container_click::Click;
use mcrs_minecraft_container_click::ClickInput;
use mcrs_minecraft_container_click::ClickKind;
use mcrs_minecraft_container_click::Clicked;
use mcrs_minecraft_container_click::Gesture;
use mcrs_minecraft_container_click::StackPresence;

const ENTRY: Clicked = Clicked::Entry(12);

const BASE: ClickInput = ClickInput {
    gesture: Gesture::OffhandKey,
    shift: false,
    control: false,
    creative: false,
    cursor: StackPresence::Empty,
    target: StackPresence::Stack,
};

const fn press(button: u8) -> Gesture {
    Gesture::Press {
        button,
        binding: ButtonBinding::Other,
    }
}

const fn release(button: u8) -> Gesture {
    Gesture::Release {
        button,
        binding: ButtonBinding::Other,
    }
}

const fn pick_press(button: u8) -> Gesture {
    Gesture::Press {
        button,
        binding: ButtonBinding::PickItem,
    }
}

const fn pick_release(button: u8) -> Gesture {
    Gesture::Release {
        button,
        binding: ButtonBinding::PickItem,
    }
}

const fn gives(kind: ClickKind, button: u8, clicked: Clicked) -> Option<Click> {
    Some(Click {
        kind,
        button,
        clicked,
    })
}

struct Case {
    name: &'static str,
    clicked: Clicked,
    input: ClickInput,
    expected: Option<Click>,
}

const HELD: StackPresence = StackPresence::Stack;

const CASES: [Case; 32] = [
    Case {
        name: "press of the middle button without pick",
        clicked: ENTRY,
        input: ClickInput {
            gesture: press(2),
            ..BASE
        },
        expected: None,
    },
    Case {
        name: "press of the pick button without creative",
        clicked: ENTRY,
        input: ClickInput {
            gesture: pick_press(2),
            ..BASE
        },
        expected: None,
    },
    Case {
        name: "press of the left button in the empty area",
        clicked: Clicked::Nowhere,
        input: ClickInput {
            gesture: press(0),
            ..BASE
        },
        expected: None,
    },
    Case {
        name: "press of pick with creative in the empty area",
        clicked: Clicked::Nowhere,
        input: ClickInput {
            gesture: pick_press(2),
            creative: true,
            ..BASE
        },
        expected: None,
    },
    Case {
        name: "press of pick with creative on an entry",
        clicked: ENTRY,
        input: ClickInput {
            gesture: pick_press(2),
            creative: true,
            ..BASE
        },
        expected: gives(ClickKind::Clone, 2, ENTRY),
    },
    Case {
        name: "press of pick with creative outside the window",
        clicked: Clicked::Outside,
        input: ClickInput {
            gesture: pick_press(2),
            creative: true,
            ..BASE
        },
        expected: gives(ClickKind::Clone, 2, Clicked::Outside),
    },
    Case {
        name: "press of a left button bound to pick with creative and Shift",
        clicked: ENTRY,
        input: ClickInput {
            gesture: pick_press(0),
            creative: true,
            shift: true,
            ..BASE
        },
        expected: gives(ClickKind::Clone, 0, ENTRY),
    },
    Case {
        name: "press of the right button outside the window with Shift",
        clicked: Clicked::Outside,
        input: ClickInput {
            gesture: press(1),
            shift: true,
            ..BASE
        },
        expected: gives(ClickKind::Throw, 1, Clicked::Outside),
    },
    Case {
        name: "press of the left button with Shift",
        clicked: ENTRY,
        input: ClickInput {
            gesture: press(0),
            shift: true,
            ..BASE
        },
        expected: gives(ClickKind::QuickMove, 0, ENTRY),
    },
    Case {
        name: "press of the right button on an empty entry",
        clicked: ENTRY,
        input: ClickInput {
            gesture: press(1),
            target: StackPresence::Empty,
            ..BASE
        },
        expected: gives(ClickKind::PickUp, 1, ENTRY),
    },
    Case {
        name: "press of the left button with a cursor stack",
        clicked: ENTRY,
        input: ClickInput {
            gesture: press(0),
            cursor: HELD,
            ..BASE
        },
        expected: None,
    },
    Case {
        name: "release of the pick button without creative",
        clicked: ENTRY,
        input: ClickInput {
            gesture: pick_release(2),
            cursor: HELD,
            ..BASE
        },
        expected: gives(ClickKind::Clone, 2, ENTRY),
    },
    Case {
        name: "release of the left button outside the window with Shift",
        clicked: Clicked::Outside,
        input: ClickInput {
            gesture: release(0),
            cursor: HELD,
            shift: true,
            ..BASE
        },
        expected: gives(ClickKind::PickUp, 0, Clicked::Outside),
    },
    Case {
        name: "release of the right button with Shift",
        clicked: ENTRY,
        input: ClickInput {
            gesture: release(1),
            cursor: HELD,
            shift: true,
            ..BASE
        },
        expected: gives(ClickKind::QuickMove, 1, ENTRY),
    },
    Case {
        name: "release of the fourth button",
        clicked: ENTRY,
        input: ClickInput {
            gesture: release(4),
            cursor: HELD,
            ..BASE
        },
        expected: gives(ClickKind::PickUp, 4, ENTRY),
    },
    Case {
        name: "release of the left button in the empty area",
        clicked: Clicked::Nowhere,
        input: ClickInput {
            gesture: release(0),
            cursor: HELD,
            ..BASE
        },
        expected: gives(ClickKind::PickUp, 0, Clicked::Nowhere),
    },
    Case {
        name: "release of the left button with the empty cursor stack",
        clicked: ENTRY,
        input: ClickInput {
            gesture: release(0),
            ..BASE
        },
        expected: None,
    },
    Case {
        name: "off-hand key",
        clicked: ENTRY,
        input: BASE,
        expected: gives(ClickKind::Swap, 40, ENTRY),
    },
    Case {
        name: "first hotbar key on an empty entry",
        clicked: ENTRY,
        input: ClickInput {
            gesture: Gesture::HotbarKey(1),
            target: StackPresence::Empty,
            ..BASE
        },
        expected: gives(ClickKind::Swap, 0, ENTRY),
    },
    Case {
        name: "ninth hotbar key",
        clicked: ENTRY,
        input: ClickInput {
            gesture: Gesture::HotbarKey(9),
            ..BASE
        },
        expected: gives(ClickKind::Swap, 8, ENTRY),
    },
    Case {
        name: "hotbar key zero",
        clicked: ENTRY,
        input: ClickInput {
            gesture: Gesture::HotbarKey(0),
            ..BASE
        },
        expected: None,
    },
    Case {
        name: "hotbar key ten",
        clicked: ENTRY,
        input: ClickInput {
            gesture: Gesture::HotbarKey(10),
            ..BASE
        },
        expected: None,
    },
    Case {
        name: "off-hand key with a cursor stack",
        clicked: ENTRY,
        input: ClickInput {
            cursor: HELD,
            ..BASE
        },
        expected: None,
    },
    Case {
        name: "hotbar key outside the window",
        clicked: Clicked::Outside,
        input: ClickInput {
            gesture: Gesture::HotbarKey(3),
            ..BASE
        },
        expected: None,
    },
    Case {
        name: "off-hand key in the empty area",
        clicked: Clicked::Nowhere,
        input: BASE,
        expected: None,
    },
    Case {
        name: "pick key with creative and a cursor stack",
        clicked: ENTRY,
        input: ClickInput {
            gesture: Gesture::PickKey,
            creative: true,
            cursor: HELD,
            ..BASE
        },
        expected: gives(ClickKind::Clone, 0, ENTRY),
    },
    Case {
        name: "pick key without creative",
        clicked: ENTRY,
        input: ClickInput {
            gesture: Gesture::PickKey,
            ..BASE
        },
        expected: None,
    },
    Case {
        name: "pick key with creative on an empty entry",
        clicked: ENTRY,
        input: ClickInput {
            gesture: Gesture::PickKey,
            creative: true,
            target: StackPresence::Empty,
            ..BASE
        },
        expected: None,
    },
    Case {
        name: "drop key with Control",
        clicked: ENTRY,
        input: ClickInput {
            gesture: Gesture::DropKey,
            control: true,
            ..BASE
        },
        expected: gives(ClickKind::Throw, 1, ENTRY),
    },
    Case {
        name: "drop key with a cursor stack",
        clicked: ENTRY,
        input: ClickInput {
            gesture: Gesture::DropKey,
            cursor: HELD,
            ..BASE
        },
        expected: gives(ClickKind::Throw, 0, ENTRY),
    },
    Case {
        name: "drop key on an empty entry",
        clicked: ENTRY,
        input: ClickInput {
            gesture: Gesture::DropKey,
            target: StackPresence::Empty,
            ..BASE
        },
        expected: None,
    },
    Case {
        name: "drop key outside the window",
        clicked: Clicked::Outside,
        input: ClickInput {
            gesture: Gesture::DropKey,
            ..BASE
        },
        expected: None,
    },
];

#[test]
fn each_gesture_gives_the_click_of_its_rule() {
    for case in CASES {
        assert_eq!(
            Click::from_input(case.clicked, case.input),
            case.expected,
            "{}",
            case.name
        );
    }
}
