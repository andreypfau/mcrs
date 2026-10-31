//! A command tree, a parser, suggestions and an executor that agree with Brigadier. A parse,
//! a suggestion pass and a run borrow the command line and reuse the buffers of the caller, so
//! they allocate nothing in the steady state.

#![no_std]

extern crate alloc;

mod argument;
mod argument_handle;
mod build_error;
mod contains;
mod error;
mod exec;
mod fork_mode;
mod graph;
mod graph_builder;
mod node;
mod node_id;
mod node_kind;
mod parse;
mod reader;
mod requirement;
mod span;
mod suggest;
mod usage;
mod whitespace;

pub use argument::{
    Argument, ArgumentSet, BoolArgument, DoubleArgument, FloatArgument, IntegerArgument,
    LongArgument, NumberArgument, StringArgument,
};
pub use argument_handle::ArgumentHandle;
pub use build_error::BuildError;
pub use contains::Contains;
pub use error::{ErrorKind, LibraryError, SyntaxError};
pub use exec::{Chain, End, Executor, Failure, Limits, SourceLists, Stage, execute};
pub use fork_mode::ForkMode;
pub use graph::Graph;
pub use graph_builder::GraphBuilder;
pub use node::Node;
pub use node_id::NodeId;
pub use node_kind::NodeKind;
pub use parse::{ContextView, ParseResult, ParseScratch, SuggestionParent};
pub use reader::{Reader, SEPARATOR, StringSpan, StringValue};
pub use requirement::Requirement;
pub use span::Span;
pub use suggest::{
    Suggestion, SuggestionBuilder, SuggestionList, SuggestionText, eq_ignore_case, recycle,
    starts_with_ignore_case,
};
pub use usage::{Presence, UsageDepth};
pub use whitespace::is_whitespace;
