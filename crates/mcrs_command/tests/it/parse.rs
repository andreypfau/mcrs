use crate::{Builder, OP, OPEN, TestArgument, TestGraph, executable_literal};
use mcrs_command::{
    Argument, BuildError, Chain, IntegerArgument, LibraryError, NodeId, ParseScratch, Reader,
    StringArgument, SyntaxError,
};
use std::convert::Infallible;

fn reported(
    graph: &TestGraph,
    input: &str,
    level: u8,
) -> Result<Vec<NodeId>, SyntaxError<Infallible>> {
    let mut scratch = ParseScratch::default();
    let parse = graph.parse(input, 0, &level, &mut scratch);
    parse
        .check(graph)
        .map(|(context, _)| context.nodes().map(|(node, _)| node).collect())
}

#[test]
fn the_reported_error_follows_the_validity_rules() {
    let mut b = Builder::new();
    let a = executable_literal(&mut b, NodeId::ROOT, "a");
    let n = b.argument(a, "n", IntegerArgument::ANY, OPEN).unwrap();
    b.executes(n.node(), 0).unwrap();
    let c = b.literal(NodeId::ROOT, "c", OPEN).unwrap();
    for (name, argument) in [
        ("n", TestArgument::Integer(IntegerArgument::ANY)),
        ("w", TestArgument::String(StringArgument::Word)),
    ] {
        let node = b.argument(c, name, argument, OPEN).unwrap();
        b.executes(node.node(), 0).unwrap();
    }
    b.literal(NodeId::ROOT, "d", OPEN).unwrap();
    let t = b.literal(NodeId::ROOT, "t", OPEN).unwrap();
    b.redirect(t, a).unwrap();
    let graph = b.freeze().unwrap();

    let cases: [(&str, SyntaxError<Infallible>); 9] = [
        ("b", SyntaxError::library(LibraryError::UnknownCommand, 0)),
        ("ab", SyntaxError::library(LibraryError::UnknownCommand, 0)),
        ("A", SyntaxError::library(LibraryError::UnknownCommand, 0)),
        ("a x", SyntaxError::library(LibraryError::ExpectedInt, 2)),
        (
            "a 5x",
            SyntaxError::library(LibraryError::ExpectedSeparator, 3),
        ),
        (
            "c !",
            SyntaxError::library(LibraryError::UnknownArgument, 2),
        ),
        ("d", SyntaxError::library(LibraryError::UnknownCommand, 1)),
        ("a ", SyntaxError::library(LibraryError::UnknownArgument, 1)),
        ("t ", SyntaxError::library(LibraryError::UnknownCommand, 2)),
    ];
    for (input, expected) in cases {
        assert_eq!(reported(&graph, input, 0), Err(expected), "{input}");
    }
    assert_eq!(reported(&graph, "a 5", 0), Ok(vec![a, n.node()]));
    assert_eq!(reported(&graph, "t 5", 0), Ok(vec![n.node()]));
}

#[test]
fn a_parse_to_the_end_beats_the_earlier_sibling_that_leaves_text() {
    let mut b = Builder::new();
    let tp = b.literal(NodeId::ROOT, "tp", OPEN).unwrap();
    let destination = b
        .argument(tp, "destination", StringArgument::Word, OPEN)
        .unwrap();
    b.executes(destination.node(), 1).unwrap();
    let targets = b
        .argument(tp, "targets", StringArgument::Word, OPEN)
        .unwrap();
    let location = b
        .argument(targets.node(), "location", IntegerArgument::ANY, OPEN)
        .unwrap();
    b.executes(location.node(), 2).unwrap();
    let graph = b.freeze().unwrap();

    assert_eq!(
        reported(&graph, "tp Steve", 0),
        Ok(vec![tp, destination.node()])
    );
    assert_eq!(
        reported(&graph, "tp Steve 3", 0),
        Ok(vec![tp, targets.node(), location.node()])
    );

    let mut scratch = ParseScratch::default();
    let parse = graph.parse("tp Steve 3", 0, &0, &mut scratch);
    let (context, _) = parse.check(&graph).unwrap();
    let (_, span) = context
        .nodes()
        .find(|&(node, _)| node == location.node())
        .unwrap();
    let value = graph.value("tp Steve 3", span, location, &0u8).unwrap();
    assert_eq!(value, Ok(3));
}

#[test]
fn requirements_hide_nodes_from_the_parse() {
    let mut b = Builder::new();
    let gamemode = b.literal(NodeId::ROOT, "gamemode", OP).unwrap();
    b.executes(gamemode, 0).unwrap();
    let say = b.literal(NodeId::ROOT, "say", OPEN).unwrap();
    let secret = b.literal(say, "secret", OP).unwrap();
    b.executes(secret, 0).unwrap();
    let word = b.argument(say, "word", StringArgument::Word, OPEN).unwrap();
    b.executes(word.node(), 0).unwrap();
    let alias = b.literal(NodeId::ROOT, "s", OPEN).unwrap();
    b.redirect(alias, secret).unwrap();
    let graph = b.freeze().unwrap();

    assert_eq!(
        reported(&graph, "gamemode", 0),
        Err(SyntaxError::library(LibraryError::UnknownCommand, 0))
    );
    assert_eq!(reported(&graph, "gamemode", 2), Ok(vec![gamemode]));
    // A literal the source cannot use does not keep its argument siblings out.
    assert_eq!(
        reported(&graph, "say secret", 0),
        Ok(vec![say, word.node()])
    );
    assert_eq!(reported(&graph, "say secret", 2), Ok(vec![say, secret]));
    // A redirect to a node the source cannot use is not followed.
    assert_eq!(
        reported(&graph, "s x", 0),
        Err(SyntaxError::library(LibraryError::UnknownArgument, 2))
    );
}

#[test]
fn building_rejects_merges_and_redirects_that_conflict() {
    let mut b = Builder::new();
    let a = b.literal(NodeId::ROOT, "a", OPEN).unwrap();
    assert_eq!(b.literal(NodeId::ROOT, "a", OPEN), Ok(a));
    assert_eq!(
        b.literal(NodeId::ROOT, "a", OP),
        Err(BuildError::Conflict {
            parent: NodeId::ROOT,
            existing: a
        })
    );
    assert!(matches!(
        b.argument(NodeId::ROOT, "a", StringArgument::Word, OPEN),
        Err(BuildError::Conflict { .. })
    ));
    let x = b.literal(NodeId::ROOT, "x", OPEN).unwrap();
    let y = b.literal(NodeId::ROOT, "y", OPEN).unwrap();
    b.redirect(x, y).unwrap();
    assert_eq!(
        b.literal(x, "child", OPEN),
        Err(BuildError::RedirectWithChildren(x))
    );
    b.redirect(y, x).unwrap();
    assert!(matches!(b.freeze(), Err(BuildError::RedirectCycle(_))));
}

#[test]
fn an_integer_out_of_range_fails_at_its_start_with_the_bound() {
    let range = IntegerArgument { min: 1, max: 5 };
    let cases: [(&str, Result<i32, SyntaxError<Infallible>>, usize); 3] = [
        ("x 3", Ok(3), 3),
        (
            "x 0",
            Err(SyntaxError::library(
                LibraryError::IntegerTooLow { value: 0, min: 1 },
                2,
            )),
            2,
        ),
        (
            "x 60",
            Err(SyntaxError::library(
                LibraryError::IntegerTooHigh { value: 60, max: 5 },
                2,
            )),
            2,
        ),
    ];
    for (input, expected, cursor) in cases {
        let mut reader = Reader::at(input, 2);
        assert_eq!(range.parse(&mut reader, &0u8), expected, "{input}");
        assert_eq!(reader.cursor(), cursor, "{input}");
    }
}

#[test]
fn a_merge_replaces_the_command_and_adds_the_children_to_the_kept_node() {
    let command = |graph: &TestGraph, input: &str| {
        let mut scratch = ParseScratch::default();
        let parse = graph.parse(input, 0, &0u8, &mut scratch);
        parse.check(graph).map(|(_, command)| *command)
    };
    let mut b = Builder::new();
    let first = b.literal(NodeId::ROOT, "a", OPEN).unwrap();
    b.executes(first, 1).unwrap();
    b.literal(NodeId::ROOT, "z", OPEN).unwrap();
    let again = b.literal(NodeId::ROOT, "a", OPEN).unwrap();
    b.executes(again, 2).unwrap();
    let child = b.literal(again, "x", OPEN).unwrap();
    b.executes(child, 3).unwrap();
    let graph = b.freeze().unwrap();

    assert_eq!(again, first);
    assert_eq!(command(&graph, "a"), Ok(2));
    assert_eq!(command(&graph, "a x"), Ok(3));
    let root = graph.node(NodeId::ROOT).unwrap();
    assert_eq!(root.children().len(), 2);
    assert_eq!(root.children().first(), Some(&first));
}

#[test]
fn each_context_of_a_chain_keeps_its_own_value_of_a_name() {
    let mut b = Builder::new();
    let a = b.literal(NodeId::ROOT, "a", OPEN).unwrap();
    let n = b.argument(a, "n", IntegerArgument::ANY, OPEN).unwrap();
    b.executes(n.node(), 0).unwrap();
    b.redirect(n.node(), a).unwrap();
    let graph = b.freeze().unwrap();

    let input = "a 1 2";
    let mut scratch = ParseScratch::default();
    let parse = graph.parse(input, 0, &0u8, &mut scratch);
    parse.check(&graph).unwrap();
    let mut chain = Chain::default();
    chain.copy_from(&graph, &parse);
    assert_eq!(chain.stages().len(), 2);
    let value = |stage| {
        let span = chain.span(stage, n.node()).unwrap();
        graph.value(input, span, n, &0u8).unwrap()
    };
    assert_eq!((value(0), value(1)), (Ok(1), Ok(2)));
}
