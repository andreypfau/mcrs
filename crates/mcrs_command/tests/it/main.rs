#![expect(clippy::unwrap_used, reason = "a test reports a failure by panicking")]

mod alloc_free;
mod exec;
mod parse;
mod reader;
mod suggest;

use mcrs_command::{
    Argument, ArgumentSet, BoolArgument, Contains, Graph, GraphBuilder, IntegerArgument, NodeId,
    Reader, Requirement, StringArgument, SuggestionBuilder, SyntaxError,
};
use std::convert::Infallible;

/// The source is a permission level; a node requires one.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Level(pub u8);

impl Requirement<u8> for Level {
    fn test(&self, source: &u8) -> bool {
        *source >= self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TestArgument {
    Bool(BoolArgument),
    Integer(IntegerArgument),
    String(StringArgument),
}

impl From<IntegerArgument> for TestArgument {
    fn from(value: IntegerArgument) -> Self {
        TestArgument::Integer(value)
    }
}

impl From<StringArgument> for TestArgument {
    fn from(value: StringArgument) -> Self {
        TestArgument::String(value)
    }
}

impl From<BoolArgument> for TestArgument {
    fn from(value: BoolArgument) -> Self {
        TestArgument::Bool(value)
    }
}

impl Contains<IntegerArgument> for TestArgument {
    fn get(&self) -> Option<&IntegerArgument> {
        match self {
            TestArgument::Integer(argument) => Some(argument),
            _ => None,
        }
    }
}

impl Contains<StringArgument> for TestArgument {
    fn get(&self) -> Option<&StringArgument> {
        match self {
            TestArgument::String(argument) => Some(argument),
            _ => None,
        }
    }
}

impl ArgumentSet<u8> for TestArgument {
    type Error = Infallible;
    type Tooltip = ();

    fn check(&self, reader: &mut Reader<'_>, source: &u8) -> Result<(), SyntaxError<Infallible>> {
        match self {
            TestArgument::Bool(argument) => argument.parse(reader, source).map(drop),
            TestArgument::Integer(argument) => argument.parse(reader, source).map(drop),
            TestArgument::String(argument) => argument.parse(reader, source).map(drop),
        }
    }

    fn suggest<'t>(
        &'t self,
        _: &'t u8,
        _: &mcrs_command::ContextView<'_>,
        builder: &mut SuggestionBuilder<'t, '_, ()>,
    ) {
        if let TestArgument::Bool(_) = self {
            BoolArgument::suggest(builder);
        }
    }
}

pub type TestGraph = Graph<TestArgument, Level, u32>;
pub type Builder = GraphBuilder<TestArgument, Level, u32>;

pub const OPEN: Level = Level(0);
pub const OP: Level = Level(2);

pub fn executable_literal(builder: &mut Builder, parent: NodeId, name: &str) -> NodeId {
    let node = builder.literal(parent, name, OPEN).unwrap();
    builder.executes(node, 0).unwrap();
    node
}
