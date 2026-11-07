//! Molang for every host of mcrs: a parser, a typed SSA form, passes that keep every result
//! bit, a register bytecode, a scalar VM and a batch VM over the same bytecode.

#![no_std]

extern crate alloc;

mod batch;
mod batch_subject;
mod bytecode;
mod channel_class;
mod compile;
mod diagnostic;
mod emit;
mod input_range;
mod ir;
mod layout;
mod lexer;
mod lower;
mod lower_branch;
mod lower_call;
mod math;
mod parse;
mod pass;
mod pure_program;
mod query;
mod resource;
mod scalar;
mod subject;
mod syntax;
mod value;

pub use batch::BatchVm;
pub use batch_subject::{BatchOutput, BatchQueries, BatchSubjects, BatchVariables};
pub use bytecode::Program;
pub use channel_class::{ChannelClass, StateReason};
pub use compile::{CompileOptions, Compiled, Expression, Source, compile, compile_text};
pub use diagnostic::{Diagnostic, ErrorKind, Span, Warning, WarningKind};
pub use input_range::InputRange;
pub use layout::{Bank, TypeConflict, VariableLayout, VariableSlot};
pub use math::{BinaryFn, EaseDirection, EaseShape, Easing, TernaryFn, UnaryFn};
pub use parse::parse;
pub use pure_program::{PureInput, PureInstr, PureProgram, Register};
pub use query::{
    Argument, Arguments, Literal, LiteralKey, QueryBinding, QueryDeclaration, QueryError,
    QueryHost, QueryRegistry, QueryScope, QuerySlot, Resolution, SlotKey, UseContext,
};
pub use resource::{ResourceArray, ResourceSet};
pub use scalar::ScalarVm;
pub use subject::{QueryValues, Subject, Variables};
pub use syntax::{BinaryOp, Body, Expr, ExprKind, Stmt, StmtKind, Syntax, UnaryOp, VariableSpace};
pub use value::{Milestone, Rate, Resource, ResourceKind, Value, ValueType, string_hash};
