use crate::{Builder, OP, OPEN, TestGraph};
use mcrs_command::{
    BoolArgument, NodeId, ParseScratch, Suggestion, SuggestionList, SuggestionText,
};
use mcrs_text_span::Span;

fn suggestions(graph: &TestGraph, input: &str, level: u8) -> (Span, Vec<String>) {
    let mut scratch = ParseScratch::default();
    let parse = graph.parse(input, 0, &level, &mut scratch);
    let mut out = Vec::new();
    graph.suggest(&parse, &level, &mut out);
    let list = SuggestionList::build(input, &mut out);
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
        Suggestion::new(4, SuggestionText::new("abc"), None),
        Suggestion::new(6, SuggestionText::new("Z"), None),
        Suggestion::new(4, SuggestionText::concat("ab", "c"), None),
        Suggestion::new(4, SuggestionText::new("ABD"), None),
    ];
    let list = SuggestionList::build(input, &mut out);
    assert_eq!(list.span, Span::new(4, 6));
    let texts: Vec<String> = list
        .iter()
        .map(|(text, _)| text.chars().collect())
        .collect();
    assert_eq!(texts, ["abc", "ABD", "abZ"]);
}

#[test]
fn suggestions_after_a_redirect_come_from_the_children_of_its_target() {
    let mut b = Builder::new();
    let a = b.literal(NodeId::ROOT, "a", OPEN).unwrap();
    b.argument(a, "value", BoolArgument, OPEN).unwrap();
    let t = b.literal(NodeId::ROOT, "t", OPEN).unwrap();
    b.redirect(t, a).unwrap();
    let x = b.literal(NodeId::ROOT, "x", OPEN).unwrap();
    let fork = b.literal(x, "fork", OPEN).unwrap();
    b.redirect(fork, x).unwrap();
    let run = b.literal(x, "run", OPEN).unwrap();
    b.redirect(run, NodeId::ROOT).unwrap();
    let secret = b.literal(NodeId::ROOT, "secret", OP).unwrap();
    b.argument(secret, "value", BoolArgument, OPEN).unwrap();
    let hidden = b.literal(NodeId::ROOT, "h", OPEN).unwrap();
    b.redirect(hidden, secret).unwrap();
    let graph = b.freeze().unwrap();

    let words = |words: &[&str]| {
        words
            .iter()
            .map(|word| word.to_string())
            .collect::<Vec<_>>()
    };
    let cases = [
        ("t ", 0, (Span::new(2, 2), words(&["false", "true"]))),
        ("t f", 0, (Span::new(2, 3), words(&["false"]))),
        ("x fork ", 0, (Span::new(7, 7), words(&["fork", "run"]))),
        ("x fork fork r", 0, (Span::new(12, 13), words(&["run"]))),
        ("x run ", 0, (Span::new(6, 6), words(&["a", "h", "t", "x"]))),
        ("h ", 2, (Span::new(2, 2), words(&["false", "true"]))),
    ];
    for (input, level, expected) in cases {
        assert_eq!(suggestions(&graph, input, level), expected, "{input}");
    }
    assert_eq!(suggestions(&graph, "h ", 0).1, Vec::<String>::new());
}
