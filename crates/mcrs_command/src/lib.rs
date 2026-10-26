#![no_std]

extern crate alloc;

pub mod argument;
pub mod error;
pub mod exec;
pub mod parse;
pub mod reader;
pub mod suggest;
pub mod tree;

pub use argument::{
    Argument, ArgumentSet, BoolArgument, DoubleArgument, FloatArgument, IntegerArgument,
    LongArgument, StringArgument,
};
pub use error::{ErrorKind, LibraryError, Span, StringSpan, SyntaxError, utf16_position};
pub use exec::{Chain, End, Executor, Failure, Limits, SourceLists, Stage, execute};
pub use parse::{ContextView, ParseResult, ParseScratch};
pub use reader::{Reader, StringValue};
pub use suggest::{Suggestion, SuggestionBuilder, SuggestionList, SuggestionText, build_list};
pub use tree::{
    Arg, BuildError, Contains, Graph, GraphBuilder, Node, NodeId, NodeKind, Requirement,
};
