use crate::{Tree, Wire, context, greedy, source};
use mcrs_command::Span;
use mcrs_command::{ErrorKind, LibraryError};
use mcrs_minecraft_chat::{
    ChatScreen, CommandInput, Highlight, Hint, History, InputStyle, Key, Request,
};
use mcrs_minecraft_command_vanilla::ClientGraph;
use mcrs_minecraft_keys::command_argument_type as parser;
use mcrs_minecraft_protocol::command::{CommandSuggestion, ParserProperties};
use std::borrow::Cow;

fn usage_lines(input: &CommandInput) -> Vec<&str> {
    input
        .hints()
        .iter()
        .map(|hint| match hint {
            Hint::Usage(range) => input.usage_text(range.clone()),
            other => panic!("{other:?}"),
        })
        .collect()
}

/// `tp` as CCMD-HINT-003 shows it.
fn tp() -> ClientGraph {
    let mut t = Tree::new();
    let tp = t.add(0, Wire::Literal("tp"), false);
    let entity = ParserProperties::Entity {
        single: true,
        players_only: false,
    };
    t.add(
        tp,
        Wire::Argument("destination", parser::ENTITY, entity, None),
        true,
    );
    let location = t.add(
        tp,
        Wire::Argument("location", parser::VEC3, ParserProperties::None, None),
        true,
    );
    t.add(
        location,
        Wire::Argument("rotation", parser::ROTATION, ParserProperties::None, None),
        true,
    );
    t.add(location, Wire::Literal("facing"), false);
    t.build()
}

#[test]
fn usage_lines_name_the_argument_children_of_the_suggestion_parent() {
    let tree = tp();
    let source = source();
    let cx = context(&tree, &source);
    let mut input = CommandInput::default();
    assert_eq!(input.refresh("/tp ", 4, &cx), None);
    assert!(input.is_ready());
    assert_eq!(
        usage_lines(&input),
        ["<destination>", "<location> [<rotation>|facing]"]
    );
}

#[test]
fn an_unknown_command_gives_one_error_line_and_a_known_one_none() {
    let mut t = Tree::new();
    t.add(0, Wire::Literal("a"), true);
    let tree = t.build();
    let source = source();
    let cx = context(&tree, &source);
    let mut input = CommandInput::default();
    input.refresh("/b", 2, &cx);
    let [Hint::Error(error)] = input.hints() else {
        panic!("{:?}", input.hints());
    };
    assert_eq!(error.kind, ErrorKind::Library(LibraryError::UnknownCommand));
    assert_eq!(error.position, 1);
    input.refresh("/a", 2, &cx);
    assert_eq!(input.hints(), []);
}

#[test]
fn arguments_of_the_last_context_take_the_colour_cycle_and_trailing_text_is_red() {
    let mut t = Tree::new();
    let execute = t.add(0, Wire::Literal("execute"), false);
    let entity = ParserProperties::Entity {
        single: false,
        players_only: false,
    };
    let at = t.add(execute, Wire::Literal("as"), false);
    let targets = t.add(
        at,
        Wire::Argument("targets", parser::ENTITY, entity.clone(), None),
        false,
    );
    t.redirect(targets, execute);
    let run = t.add(execute, Wire::Literal("run"), false);
    t.redirect(run, 0);
    let give = t.add(0, Wire::Literal("give"), false);
    let targets = t.add(
        give,
        Wire::Argument("targets", parser::ENTITY, entity, None),
        false,
    );
    let item = t.add(
        targets,
        Wire::Argument(
            "item",
            parser::RESOURCE_LOCATION,
            ParserProperties::None,
            None,
        ),
        true,
    );
    t.add(
        item,
        Wire::Argument(
            "count",
            parser::BRIGADIER_INTEGER,
            ParserProperties::Integer(mcrs_minecraft_protocol::command::NumberRange {
                min: None,
                max: None,
            }),
            None,
        ),
        true,
    );
    let tree = t.build();
    let source = source();
    let cx = context(&tree, &source);
    let mut input = CommandInput::default();

    let line = "/give @s stone 5";
    input.refresh(line, line.len(), &cx);
    assert_eq!(
        input.highlights(),
        [
            (Span::new(6, 8), Highlight::Argument(0)),
            (Span::new(9, 14), Highlight::Argument(1)),
            (Span::new(15, 16), Highlight::Argument(2)),
        ]
    );

    let line = "/execute as @s run give @s stone x";
    input.refresh(line, line.len(), &cx);
    assert_eq!(
        input.highlights(),
        [
            (Span::new(24, 26), Highlight::Argument(0)),
            (Span::new(27, 32), Highlight::Argument(1)),
            (Span::new(33, 34), Highlight::Trailing),
        ]
    );
}

fn reply(
    input: &mut CommandInput,
    cx: &mcrs_minecraft_chat::Context<'_, crate::Players>,
    id: i32,
    texts: &[&'static str],
) -> bool {
    let suggestions: Vec<_> = texts
        .iter()
        .map(|&text| CommandSuggestion {
            text: Cow::Borrowed(text),
            tooltip: None,
        })
        .collect();
    input.reply(
        id,
        3,
        input.input().len().saturating_sub(3),
        &suggestions,
        cx,
    )
}

#[test]
fn request_ids_grow_until_a_reply_and_a_stale_reply_changes_nothing() {
    let mut t = Tree::new();
    let x = t.add(0, Wire::Literal("x"), false);
    t.add(
        x,
        Wire::Argument(
            "rest",
            parser::BRIGADIER_STRING,
            greedy(),
            Some("minecraft:ask_server"),
        ),
        true,
    );
    t.add(0, Wire::Literal("y"), true);
    let tree = t.build();
    let source = source();
    let cx = context(&tree, &source);
    let mut input = CommandInput::default();

    assert_eq!(
        input.refresh("/x ", 3, &cx),
        Some(Request { id: 0, end: 3 })
    );
    assert_eq!(
        input.refresh("/x a", 4, &cx),
        Some(Request { id: 1, end: 4 })
    );
    assert!(!input.is_ready(), "no list and no hint before the reply");
    assert!(
        !reply(&mut input, &cx, 0, &["old"]),
        "the reply to a replaced request"
    );
    assert!(input.suggestions().is_empty());
    assert!(reply(&mut input, &cx, 1, &["ab", "aa"]));
    assert!(input.is_ready());
    assert_eq!(
        input.suggestions().iter().collect::<Vec<_>>(),
        ["ab", "aa"],
        "the only child keeps the order of the server"
    );
    assert_eq!(
        input.refresh("/x ab", 5, &cx),
        Some(Request { id: 0, end: 5 })
    );

    input.refresh("/y", 2, &cx);
    assert_eq!(input.open_request(), None);
    assert!(!reply(&mut input, &cx, 0, &["late"]));
    assert!(input.suggestions().iter().all(|text| text != "late"));
}

#[test]
fn tab_inserts_each_entry_in_turn_and_shows_its_rest_as_ghost_text() {
    let mut t = Tree::new();
    let gamemode = t.add(0, Wire::Literal("gamemode"), false);
    t.add(
        gamemode,
        Wire::Argument("gamemode", parser::GAMEMODE, ParserProperties::None, None),
        true,
    );
    let tree = t.build();
    let source = source();
    let cx = context(&tree, &source);
    let mut history = History::default();
    let mut screen = ChatScreen::default();
    screen.open("/", InputStyle::Plain, &cx);
    assert!(
        screen.commands.list().is_none(),
        "no list until the player types"
    );

    screen.type_text("gam", &cx);
    let list = screen.commands.list().unwrap();
    assert_eq!(list.ghost(screen.input()), Some("emode"));

    screen.type_text("emode s", &cx);
    let entries: Vec<String> = screen
        .commands
        .list()
        .unwrap()
        .entries()
        .iter()
        .map(str::to_owned)
        .collect();
    assert_eq!(entries, ["spectator", "survival"]);
    screen.key(Key::Tab { back: false }, &mut history, &cx);
    assert_eq!(screen.input(), "/gamemode spectator");
    screen.key(Key::Tab { back: false }, &mut history, &cx);
    assert_eq!(screen.input(), "/gamemode survival");
    assert_eq!(
        screen.commands.hints(),
        [],
        "an insert clears the hint lines"
    );
}

#[test]
fn chat_text_suggests_player_names_only_after_tab() {
    let tree = Tree::new().build();
    let source = source();
    let cx = context(&tree, &source);
    let mut history = History::default();
    let mut screen = ChatScreen::default();
    screen.open("", InputStyle::Plain, &cx);
    screen.type_text("hi al", &cx);
    assert!(screen.commands.list().is_none());
    screen.key(Key::Tab { back: false }, &mut history, &cx);
    assert_eq!(
        screen
            .commands
            .list()
            .unwrap()
            .entries()
            .iter()
            .collect::<Vec<_>>(),
        ["Alex"]
    );
    screen.key(Key::Tab { back: false }, &mut history, &cx);
    assert_eq!(screen.input(), "hi Alex");
}
