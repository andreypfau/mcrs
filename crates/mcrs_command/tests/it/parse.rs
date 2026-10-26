use crate::{Builder, Lib, OP, OPEN, TestGraph, executable_literal};
use mcrs_command::{
    BuildError, IntegerArgument, LibraryError, NodeId, ParseScratch, StringArgument, SyntaxError,
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
        ("n", Lib::Integer(IntegerArgument::ANY)),
        ("w", Lib::String(StringArgument::Word)),
    ] {
        let node = b.argument(c, name, argument, OPEN).unwrap();
        b.executes(node.node(), 0).unwrap();
    }
    b.literal(NodeId::ROOT, "d", OPEN).unwrap();
    let t = b.literal(NodeId::ROOT, "t", OPEN).unwrap();
    b.redirect(t, a).unwrap();
    let graph = b.freeze().unwrap();

    let cases: [(&str, SyntaxError<Infallible>); 6] = [
        ("b", SyntaxError::library(LibraryError::UnknownCommand, 0)),
        ("a x", SyntaxError::library(LibraryError::ExpectedInt, 2)),
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
