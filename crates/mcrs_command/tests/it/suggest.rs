use crate::{Builder, OP, OPEN, TestGraph};
use mcrs_command::{
    BoolArgument, NodeId, ParseScratch, Span, Suggestion, SuggestionText, build_list,
};

fn suggestions(graph: &TestGraph, input: &str, level: u8) -> (Span, Vec<String>) {
    let mut scratch = ParseScratch::default();
    let parse = graph.parse(input, 0, &level, &mut scratch);
    let mut out = Vec::new();
    graph.suggest(&parse, &level, &mut out);
    let list = build_list(input, &mut out);
    let texts = list
        .iter()
        .map(|(text, _)| text.chars().collect())
        .collect();
    (list.span, texts)
}

fn graph() -> TestGraph {
    let mut b = Builder::new();
    for name in ["gamemode", "gamerule", "Give"] {
        b.literal(NodeId::ROOT, name, OP).unwrap();
    }
    let flag = b.literal(NodeId::ROOT, "flag", OPEN).unwrap();
    b.argument(flag, "value", BoolArgument, OPEN).unwrap();
    b.freeze().unwrap()
}

#[test]
fn suggestions_come_from_the_children_the_source_can_use() {
    let graph = graph();
    assert_eq!(
        suggestions(&graph, "gam", 2),
        (Span::new(0, 3), vec!["gamemode".into(), "gamerule".into()])
    );
    assert_eq!(suggestions(&graph, "gam", 0).1, Vec::<String>::new());
    assert_eq!(suggestions(&graph, "gamemode", 2).1, Vec::<String>::new());
    assert_eq!(
        suggestions(&graph, "", 2).1,
        ["flag", "gamemode", "gamerule", "Give"]
    );
    assert_eq!(
        suggestions(&graph, "flag ", 0),
        (Span::new(5, 5), vec!["false".into(), "true".into()])
    );
    assert_eq!(
        suggestions(&graph, "flag T", 0),
        (Span::new(5, 6), vec!["true".into()])
    );
}

#[test]
fn a_list_expands_each_suggestion_to_its_span_and_drops_duplicates() {
    let input = "say ab";
    let mut out: Vec<Suggestion<'_, ()>> = vec![
        Suggestion {
            start: 4,
            text: SuggestionText::new("abc"),
            tooltip: None,
        },
        Suggestion {
            start: 6,
            text: SuggestionText::new("Z"),
            tooltip: None,
        },
        Suggestion {
            start: 4,
            text: SuggestionText::concat("ab", "c"),
            tooltip: None,
        },
        Suggestion {
            start: 4,
            text: SuggestionText::new("ABD"),
            tooltip: None,
        },
    ];
    let list = build_list(input, &mut out);
    assert_eq!(list.span, Span::new(4, 6));
    let texts: Vec<String> = list
        .iter()
        .map(|(text, _)| text.chars().collect())
        .collect();
    assert_eq!(texts, ["abc", "ABD", "abZ"]);
}
