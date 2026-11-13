use mcrs_minecraft_client_widget::{
    Anchor, Caret, EditBox, GuiKey, GuiPoint, GuiRect, KeyModifiers, LengthUnit, PointerButton,
    PointerPress, Step, WidgetEvent, WidgetId, WidgetKind, Widgets,
};

use crate::screen::{button, key, mono, press, screen_of, widget};

const BOX: GuiRect = GuiRect::new(10, 20, 150, 20);
const ID: WidgetId = WidgetId(1);
const CHANGED: WidgetEvent = WidgetEvent::ValueChanged { widget: ID };

fn wide(text: &str) -> i32 {
    text.chars().map(|c| if c == 'W' { 20 } else { 6 }).sum()
}

fn editing(edit: EditBox) -> Widgets {
    let mut screen = screen_of([widget(1, BOX, WidgetKind::EditBox(edit))]);
    screen.focus_initial(Some(ID), 0, &mut Vec::new());
    screen
}

fn edit_of(screen: &Widgets) -> EditBox {
    match screen.get(ID).map(|widget| &widget.kind) {
        Some(WidgetKind::EditBox(edit)) => edit.clone(),
        _ => EditBox::default(),
    }
}

fn set(screen: &mut Widgets, value: &str, caret: usize, anchor: Anchor) {
    screen.change_edit_box(ID, &mono, &mut Vec::new(), |edit| {
        edit.set(value);
        edit.set_caret(caret, anchor);
        false
    });
}

#[test]
fn an_insert_drops_refused_characters_and_cuts_at_the_maximum_length_in_its_unit() {
    let smiles = |count| "\u{1f600}".repeat(count);
    let cases = [
        (
            "refused characters go",
            LengthUnit::CodeUnits,
            32,
            "",
            "a\u{a7}b\nc\t\u{7f}\0d".to_owned(),
            "abcd".to_owned(),
        ),
        (
            "a surrogate pair counts two code units",
            LengthUnit::CodeUnits,
            50,
            "",
            smiles(30),
            smiles(25),
        ),
        (
            "the kept text never ends inside a character",
            LengthUnit::CodeUnits,
            3,
            "",
            smiles(2),
            smiles(1),
        ),
        (
            "a surrogate pair counts one character",
            LengthUnit::Characters,
            256,
            "",
            smiles(300),
            smiles(256),
        ),
        (
            "a full box takes nothing",
            LengthUnit::CodeUnits,
            3,
            "abc",
            "d".to_owned(),
            "abc".to_owned(),
        ),
        (
            "a set cuts the value in code units",
            LengthUnit::CodeUnits,
            3,
            "a\u{1f600}b",
            String::new(),
            "a\u{1f600}".to_owned(),
        ),
        (
            "a set cuts the value in characters",
            LengthUnit::Characters,
            3,
            "a\u{1f600}bc",
            String::new(),
            "a\u{1f600}b".to_owned(),
        ),
    ];
    for (name, unit, max, before, inserted, expected) in cases {
        let mut edit = EditBox::default();
        edit.set_max_length(max, unit);
        edit.set(before);
        edit.insert(&inserted);
        assert_eq!(edit.text(), expected, "{name}");
    }
}

#[test]
fn the_word_positions_skip_the_spaces_after_a_word() {
    let cases = [(0, 5, 0), (1, 5, 0), (5, 8, 0), (8, 8, 5)];
    for (from, forward, backward) in cases {
        let mut edit = EditBox::default();
        edit.set("foo  bar");
        edit.set_caret(from, Anchor::Move);
        edit.right(Step::Word, Anchor::Move);
        assert_eq!(edit.caret(), forward, "forward from {from}");
        edit.set_caret(from, Anchor::Move);
        edit.left(Step::Word, Anchor::Move);
        assert_eq!(edit.caret(), backward, "backward from {from}");
    }
}

#[test]
fn an_edit_key_moves_or_deletes_by_a_word_with_ctrl_and_reports_a_new_value() {
    let cases = [
        (
            "Ctrl+Left",
            "foo bar baz",
            11,
            GuiKey::Left,
            "foo bar baz",
            8,
            false,
        ),
        (
            "Ctrl+Backspace",
            "foo bar baz",
            8,
            GuiKey::Backspace,
            "foo baz",
            4,
            true,
        ),
        ("Ctrl+Delete", "foo bar", 0, GuiKey::Delete, "bar", 0, true),
        (
            "Ctrl+Backspace at 0",
            "foo",
            0,
            GuiKey::Backspace,
            "foo",
            0,
            false,
        ),
    ];
    for (name, value, caret, pressed, expected, expected_caret, changed) in cases {
        let mut screen = editing(EditBox::default());
        set(&mut screen, value, caret, Anchor::Move);
        let events = key(&mut screen, pressed, KeyModifiers::CONTROL);
        let edit = edit_of(&screen);
        assert_eq!(
            (edit.text(), edit.caret()),
            (expected, expected_caret),
            "{name}"
        );
        assert_eq!(events.contains(&CHANGED), changed, "{name}: {events:?}");
    }
}

#[test]
fn the_view_moves_by_width_to_keep_the_cursor_inside() {
    let thirty = "a".repeat(30);
    let wide_end = format!("{}WW", "a".repeat(20));
    let thirty = thirty.as_str();
    let cases: [(
        &str,
        fn(&str) -> i32,
        &str,
        &[GuiKey],
        core::ops::Range<usize>,
    ); 5] = [
        ("typing past the right edge", mono, thirty, &[], 7..30),
        ("Home", mono, thirty, &[GuiKey::Home], 0..23),
        (
            "Backspace at the end keeps the view",
            mono,
            thirty,
            &[GuiKey::Backspace],
            7..29,
        ),
        (
            "Left onto the first visible position",
            mono,
            thirty,
            &[GuiKey::Left; 23],
            0..23,
        ),
        ("a wide character", wide, wide_end.as_str(), &[], 3..22),
    ];
    for (name, font, typed, keys, visible) in cases {
        let mut screen = editing(EditBox::default());
        let mut events = Vec::new();
        assert!(screen.type_text(typed, &font, &mut events), "{name}");
        for pressed in keys {
            screen.key_press(*pressed, KeyModifiers::NONE, 0, &font, &mut events);
        }
        let layout = edit_of(&screen).layout(BOX, 0, &font);
        assert_eq!(layout.visible, visible, "{name}");
    }
}

#[test]
fn a_click_puts_the_cursor_before_the_character_under_the_mouse() {
    let thirty = "a".repeat(30);
    let cases = [
        ("left of the text", "abc", 13.9, 0),
        ("at the text origin", "abc", 14.0, 0),
        ("on the right half of a", "abc", 19.5, 0),
        ("at the start of b", "abc", 20.0, 1),
        ("inside c", "abc", 31.0, 2),
        ("right of the text", "abc", 159.0, 3),
        ("left of a scrolled view", thirty.as_str(), 12.0, 7),
        ("inside a scrolled view", thirty.as_str(), 20.0, 8),
    ];
    for (name, typed, x, expected) in cases {
        let mut screen = editing(EditBox::default());
        screen.type_text(typed, &mono, &mut Vec::new());
        let mut events = Vec::new();
        screen.mouse_press(
            PointerPress {
                x,
                y: 30.0,
                button: PointerButton::Left,
                modifiers: KeyModifiers::NONE,
                now_ms: 0,
            },
            &mono,
            &mut events,
        );
        assert_eq!(edit_of(&screen).caret(), expected, "{name}");
    }
}

#[test]
fn shift_click_keeps_the_anchor_and_a_double_click_selects_a_word() {
    let shift = KeyModifiers::SHIFT;
    let none = KeyModifiers::NONE;
    let cases: [(&str, &[(f64, KeyModifiers, u64)], _); 5] = [
        ("click", &[(26.0, none, 0)], 2..2),
        ("Shift+click", &[(26.0, shift, 0)], 2..8),
        ("double click", &[(21.0, none, 0), (21.0, none, 100)], 0..5),
        (
            "Shift+double click",
            &[(21.0, shift, 0), (21.0, shift, 100)],
            0..5,
        ),
        (
            "two clicks 400 ms apart",
            &[(21.0, none, 0), (21.0, none, 400)],
            1..1,
        ),
    ];
    for (name, presses, selection) in cases {
        let mut screen = editing(EditBox::default());
        screen.type_text("foo  bar", &mono, &mut Vec::new());
        for &(x, modifiers, now_ms) in presses {
            screen.mouse_press(
                PointerPress {
                    x,
                    y: 30.0,
                    button: PointerButton::Left,
                    modifiers,
                    now_ms,
                },
                &mono,
                &mut Vec::new(),
            );
        }
        assert_eq!(edit_of(&screen).selection(), selection, "{name}");
    }
}

#[test]
fn the_cursor_blinks_in_phases_of_300_ms_from_the_focus() {
    let mut screen = screen_of([widget(1, BOX, WidgetKind::EditBox(EditBox::default()))]);
    screen.focus_initial(Some(ID), 1000, &mut Vec::new());
    screen.type_text("a", &mono, &mut Vec::new());
    screen.type_text("b", &mono, &mut Vec::new());
    let underscore = Some(Caret::Underscore(GuiPoint::new(27, 26)));
    let cases = [
        (1000, underscore),
        (1299, underscore),
        (1300, None),
        (1599, None),
        (1600, underscore),
        (1900, None),
    ];
    for (now_ms, caret) in cases {
        assert_eq!(
            edit_of(&screen).layout(BOX, now_ms, &mono).caret,
            caret,
            "at {now_ms}"
        );
    }
}

#[test]
fn copy_and_cut_give_the_selection_and_nothing_without_one() {
    #[derive(Clone, Copy, Debug)]
    enum Op {
        Copy,
        Cut,
    }
    let cases = [
        ("copy", Op::Copy, true, 2, Some("c"), "abc", false),
        (
            "copy without a selection",
            Op::Copy,
            true,
            3,
            None,
            "abc",
            false,
        ),
        ("cut", Op::Cut, true, 2, Some("c"), "ab", true),
        (
            "cut without a selection",
            Op::Cut,
            true,
            3,
            None,
            "abc",
            false,
        ),
        (
            "cut of a box that is not editable",
            Op::Cut,
            false,
            2,
            Some("c"),
            "abc",
            false,
        ),
    ];
    for (name, op, editable, anchor, clipboard, value, changed) in cases {
        let mut screen = editing(EditBox::default());
        screen.change_edit_box(ID, &mono, &mut Vec::new(), |edit| {
            edit.set("abc");
            edit.set_caret(anchor, Anchor::Keep);
            edit.editable = editable;
            false
        });
        let mut events = Vec::new();
        let copied = match op {
            Op::Copy => screen.copy().map(str::to_owned),
            Op::Cut => screen.cut(&mono, &mut events),
        };
        assert_eq!(copied.as_deref(), clipboard, "{name}");
        assert_eq!(edit_of(&screen).text(), value, "{name}");
        assert_eq!(events.contains(&CHANGED), changed, "{name}");
    }
}

#[test]
fn tab_and_activation_start_edit_mode_and_an_arrow_does_not() {
    let cases: [(&str, &[GuiKey], bool); 3] = [
        ("Tab", &[GuiKey::Tab, GuiKey::Tab], true),
        ("arrow", &[GuiKey::Tab, GuiKey::Down], false),
        (
            "arrow, then Enter",
            &[GuiKey::Tab, GuiKey::Down, GuiKey::Enter],
            true,
        ),
    ];
    for (name, keys, captures) in cases {
        let mut screen = screen_of([
            button(0, 10, 0, 150, 20),
            widget(1, BOX, WidgetKind::EditBox(EditBox::default())),
        ]);
        for pressed in keys {
            key(&mut screen, *pressed, KeyModifiers::NONE);
        }
        assert_eq!(screen.focused(), Some(ID), "{name}");
        assert_eq!(screen.captures_keyboard(), captures, "{name}");
        assert_eq!(
            screen.type_text("x", &mono, &mut Vec::new()),
            captures,
            "{name}"
        );
    }
}

#[test]
fn the_options_of_a_box_filter_cut_keep_the_focus_and_colour_it() {
    let cases: [(&str, fn(&mut EditBox), &str, bool, u32); 5] = [
        ("defaults", |_| {}, "a1b", false, 0xFFE0_E0E0),
        (
            "a filter refuses a value",
            |edit| edit.filter = Some(|value| !value.contains(char::is_numeric)),
            "ab",
            false,
            0xFFE0_E0E0,
        ),
        (
            "a width limit refuses a wider value",
            |edit| edit.width_limit = Some(12),
            "a1",
            false,
            0xFFE0_E0E0,
        ),
        (
            "a box that cannot lose the focus stays in edit mode",
            |edit| edit.can_lose_focus = false,
            "a1b",
            true,
            0xFFE0_E0E0,
        ),
        (
            "a box that is not editable takes its colour",
            |edit| {
                edit.editable = false;
                edit.uneditable_color = 0xFF12_3456;
            },
            "",
            false,
            0xFF12_3456,
        ),
    ];
    for (name, configure, text, editing_after, color) in cases {
        let mut edit = EditBox::default();
        configure(&mut edit);
        let mut screen = screen_of([
            widget(1, BOX, WidgetKind::EditBox(edit)),
            button(2, 10, 50, 150, 20),
        ]);
        screen.focus_initial(Some(ID), 0, &mut Vec::new());
        screen.type_text("a1b", &mono, &mut Vec::new());
        let edit = edit_of(&screen);
        assert_eq!(edit.text(), text, "{name}");
        assert_eq!(edit.layout(BOX, 0, &mono).color, color, "{name}");
        press(&mut screen, 20.0, 55.0, PointerButton::Left);
        assert_eq!(screen.focused(), Some(WidgetId(2)), "{name}");
        assert_eq!(edit_of(&screen).is_editing(), editing_after, "{name}");
    }
}

#[test]
fn a_paste_replaces_the_selection_and_reports_a_new_value() {
    let cases = [
        ("over the selection", true, 2, "x\ny", "abxy", true),
        ("at the cursor", true, 3, "xy", "abcxy", true),
        ("of refused characters only", true, 3, "\n", "abc", false),
        (
            "into a box that is not editable",
            false,
            2,
            "xy",
            "abc",
            false,
        ),
    ];
    for (name, editable, anchor, pasted, value, changed) in cases {
        let mut screen = editing(EditBox::default());
        set(&mut screen, "abc", anchor, Anchor::Keep);
        screen.change_edit_box(ID, &mono, &mut Vec::new(), |edit| {
            edit.editable = editable;
            false
        });
        let mut events = Vec::new();
        screen.paste(pasted, &mono, &mut events);
        assert_eq!(edit_of(&screen).text(), value, "{name}");
        assert_eq!(events.contains(&CHANGED), changed, "{name}");
    }
}
