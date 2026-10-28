use crate::{Tree, context, source};
use mcrs_minecraft_chat::{
    ChatScreen, Closed, EditBox, History, Key, Outcome, Send, keeps_draft, normalize,
};

#[test]
fn normalization_trims_and_collapses_whitespace() {
    let long = "a".repeat(300);
    for (input, sent) in [
        ("  a   b  ", "a b"),
        ("a\tb\u{3000}c", "a b c"),
        ("a\u{a0} b", "a  b"),
        ("\u{a0}hi\u{a0}", "hi"),
        ("\u{1}a\u{1f}", "a"),
        (long.as_str(), &long[..256]),
    ] {
        let mut out = String::new();
        normalize(input, &mut out);
        assert_eq!(out, sent, "{input:?}");
    }
}

#[test]
fn the_input_box_rejects_section_signs_and_control_codes_and_holds_256_units() {
    let mut edit = EditBox::default();
    edit.insert("a\u{a7}b\u{7f}c\nd");
    assert_eq!(edit.text(), "abcd");
    edit.set("");
    edit.insert(&"\u{1f600}".repeat(200));
    assert_eq!(
        edit.text().chars().count(),
        128,
        "a surrogate pair is never split"
    );
}

#[test]
fn up_and_down_recall_the_history_and_restore_the_typed_line() {
    let tree = Tree::new().build();
    let source = source();
    let cx = context(&tree, &source);
    let mut history = History::default();
    history.add("a");
    history.add("b");
    let mut screen = ChatScreen::default();
    screen.open("", false, &cx);
    screen.type_text("c", &cx);
    let mut seen = Vec::new();
    for key in [Key::Up, Key::Up, Key::Down, Key::Down] {
        screen.key(key, &mut history, &cx);
        seen.push(screen.input().to_owned());
    }
    assert_eq!(seen, ["b", "a", "b", "c"]);
    assert_eq!(screen.key(Key::Down, &mut history, &cx), Outcome::Ignored);
}

#[test]
fn commands_outlive_a_disconnect_and_chat_text_does_not() {
    let mut history = History::from_command_file("/old\n");
    assert!(!history.add("hi"));
    assert!(history.add("/help"));
    assert!(
        !history.add("/help"),
        "a repeat of the last command is not added"
    );
    history.reset();
    assert_eq!(
        (0..history.len())
            .map(|i| history.get(i).unwrap())
            .collect::<Vec<_>>(),
        ["/old", "/help"]
    );
    assert_eq!(history.command_file(), "/old\n/help\n");
}

#[test]
fn enter_sends_the_normalized_input_and_picks_the_packet() {
    let tree = Tree::new().build();
    let source = source();
    let cx = context(&tree, &source);
    let mut history = History::default();
    let mut screen = ChatScreen::default();
    for (typed, send) in [
        ("  hi  there ", Some(Send::Chat("hi there".into()))),
        (
            "/",
            Some(Send::Command {
                line: String::new(),
                signed: false,
            }),
        ),
        ("   ", None),
    ] {
        screen.open("", false, &cx);
        screen.type_text(typed, &cx);
        let Outcome::Closed(Closed::Sent { send: sent, .. }) =
            screen.key(Key::Enter, &mut history, &cx)
        else {
            panic!("{typed:?}");
        };
        assert_eq!(sent, send, "{typed:?}");
    }
}

#[test]
fn a_draft_is_kept_only_when_the_screen_closed_without_a_send() {
    let sent = Closed::Sent {
        send: None,
        history_changed: false,
    };
    for (input, closed, save, kept) in [
        ("abc", Some(&Closed::Escape), false, false),
        ("abc", Some(&Closed::Escape), true, true),
        ("abc", Some(&sent), true, false),
        ("abc", None, false, true),
        ("  ", None, true, false),
    ] {
        assert_eq!(
            keeps_draft(input, closed, save),
            kept,
            "{input:?} {closed:?} {save}"
        );
    }
}

#[test]
fn backspace_on_a_draft_clears_it() {
    let tree = Tree::new().build();
    let source = source();
    let cx = context(&tree, &source);
    let mut history = History::default();
    let mut screen = ChatScreen::default();
    screen.open("abc", true, &cx);
    screen.key(Key::Backspace { word: false }, &mut history, &cx);
    assert_eq!(screen.input(), "");
    assert!(!screen.is_draft());
}
