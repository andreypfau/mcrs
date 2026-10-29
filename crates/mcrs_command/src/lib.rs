//! A command tree, a parser, suggestions and an executor that agree with Brigadier. A parse,
//! a suggestion pass and a run borrow the command line and reuse the buffers of the caller, so
//! they allocate nothing in the steady state.

#![no_std]

extern crate alloc;

mod argument;
mod error;
mod exec;
mod graph;
mod parse;
mod reader;
mod suggest;
mod usage;

pub use argument::{
    Argument, ArgumentSet, BoolArgument, DoubleArgument, FloatArgument, IntegerArgument,
    LongArgument, NumberArgument, StringArgument,
};
pub use error::{ErrorKind, LibraryError, SyntaxError};
pub use exec::{Chain, End, Executor, Failure, Limits, SourceLists, Stage, execute};
pub use graph::{
    ArgumentHandle, BuildError, Contains, ForkMode, Graph, GraphBuilder, Node, NodeId, NodeKind,
    Requirement,
};
pub use parse::{ContextView, ParseResult, ParseScratch, SuggestionParent};
pub use reader::{Reader, SEPARATOR, StringSpan, StringValue};
pub use suggest::{
    Suggestion, SuggestionBuilder, SuggestionList, SuggestionText, eq_ignore_case, recycle,
    starts_with_ignore_case,
};
pub use usage::{Presence, UsageDepth};
