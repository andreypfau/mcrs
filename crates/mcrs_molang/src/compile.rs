use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use crate::bytecode::Program;
use crate::diagnostic::{Diagnostic, ErrorKind, Span, Warning, WarningKind};
use crate::emit;
use crate::ir::{Function, Op};
use crate::layout::VariableLayout;
use crate::lower;
use crate::parse;
use crate::pass;
use crate::query::QueryHost;
use crate::resource::ResourceSet;
use crate::scalar::ScalarVm;
use crate::subject::Subject;
use crate::value::{Milestone, Rate, Value, ValueType};

/// The first `version` of a versioned value with the modern rules.
const MODERN_MOLANG_VERSION: f64 = 7.0;

/// How the compiler treats one expression.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CompileOptions<'a> {
    pub milestone: Milestone,
    /// Runs the passes that keep every result bit. With `false`, the program is the direct
    /// lowering, the reference for the passes.
    pub optimize: bool,
    /// The resources and arrays of the render controller; `None` outside a render controller.
    pub resources: Option<&'a ResourceSet>,
}

impl Default for CompileOptions<'_> {
    fn default() -> Self {
        Self {
            milestone: Milestone::default(),
            optimize: true,
            resources: None,
        }
    }
}

/// A Molang field as JSON gives it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Source<'a> {
    Number(f64),
    Bool(bool),
    Text(&'a str),
    /// The entries of a script array, joined with no text between them.
    ScriptArray(&'a [&'a str]),
    /// An object with the member `expression`; `version` is `None` when it is absent.
    Versioned {
        expression: &'a Source<'a>,
        version: Option<f64>,
    },
    /// A JSON value of a type that a Molang field does not take.
    Other,
}

/// A compiled expression. Most expressions of a pack are constants, which cost no run.
#[derive(Clone, Debug, PartialEq)]
pub enum Expression {
    Constant(Value),
    Program(Program),
}

impl Expression {
    /// Runs the expression for one subject.
    pub fn evaluate(&self, vm: &mut ScalarVm, subject: &mut Subject<'_>) -> Value {
        match self {
            Self::Constant(value) => *value,
            Self::Program(program) => vm.run(program, subject),
        }
    }

    #[must_use]
    pub const fn as_constant(&self) -> Option<Value> {
        match self {
            Self::Constant(value) => Some(*value),
            Self::Program(_) => None,
        }
    }

    #[must_use]
    pub const fn rate(&self) -> Rate {
        match self {
            Self::Constant(_) => Rate::Constant,
            Self::Program(program) => program.rate(),
        }
    }

    #[must_use]
    pub const fn output(&self) -> ValueType {
        match self {
            Self::Constant(value) => value.value_type(),
            Self::Program(program) => program.output(),
        }
    }
}

/// The result of a compile with no load error.
#[derive(Clone, Debug, PartialEq)]
pub struct Compiled {
    pub expression: Expression,
    pub warnings: Vec<Warning>,
}

/// Compiles one Molang field. The variables that the expression names get slots in `layout`,
/// and the host resolves its queries.
pub fn compile<H: QueryHost>(
    source: &Source<'_>,
    layout: &mut VariableLayout,
    host: &mut H,
    options: &CompileOptions<'_>,
) -> Result<Compiled, Vec<Diagnostic>> {
    let mut warnings = Vec::new();
    let text: String = match *source {
        Source::Number(value) => return Ok(constant(value as f32, warnings)),
        Source::Bool(value) => return Ok(constant(if value { 1.0 } else { 0.0 }, warnings)),
        Source::Text(text) => text.into(),
        Source::ScriptArray(entries) => entries.concat(),
        Source::Versioned {
            expression,
            version,
        } => {
            let version = version.unwrap_or(0.0);
            if libm::trunc(version) != version || !version.is_finite() {
                return Err(type_error("the member `version` is not an integer"));
            }
            if version < MODERN_MOLANG_VERSION {
                warnings.push(Warning {
                    kind: WarningKind::OldMolangVersion,
                    span: Span::default(),
                    name: String::new(),
                });
            }
            match *expression {
                Source::Number(value) => return Ok(constant(value as f32, warnings)),
                Source::Bool(value) => {
                    return Ok(constant(if value { 1.0 } else { 0.0 }, warnings));
                }
                Source::Text(text) => text.into(),
                _ => {
                    return Err(type_error(
                        "the member `expression` has a type that Molang does not take",
                    ));
                }
            }
        }
        Source::Other => return Err(type_error("a JSON value that Molang does not take")),
    };
    let mut compiled = compile_text(&text, layout, host, options)?;
    warnings.append(&mut compiled.warnings);
    compiled.warnings = warnings;
    Ok(compiled)
}

/// Compiles the text of one expression.
pub fn compile_text<H: QueryHost>(
    text: &str,
    layout: &mut VariableLayout,
    host: &mut H,
    options: &CompileOptions<'_>,
) -> Result<Compiled, Vec<Diagnostic>> {
    let syntax = parse::parse(text)?;
    let lowered = lower::lower(&syntax, layout, host, options)?;
    let mut function = lowered.function;
    if options.optimize {
        pass::optimize(&mut function);
    }
    let expression = match constant_result(&function) {
        Some(value) => Expression::Constant(value),
        None => Expression::Program(emit::emit(&function)),
    };
    Ok(Compiled {
        expression,
        warnings: lowered.warnings,
    })
}

fn constant(value: f32, warnings: Vec<Warning>) -> Compiled {
    Compiled {
        expression: Expression::Constant(Value::Number(value)),
        warnings,
    }
}

fn type_error(message: &str) -> Vec<Diagnostic> {
    vec![Diagnostic::new(ErrorKind::Type, Span::default(), message)]
}

/// The value of a body that only returns a constant.
fn constant_result(function: &Function) -> Option<Value> {
    let (last, rest) = function.body.insts.split_last()?;
    let all_constants = rest
        .iter()
        .all(|id| matches!(function.op(*id), Some(Op::Const(_) | Op::ConstBits(_))));
    let Some(Op::Return(result)) = function.op(*last) else {
        return None;
    };
    if !all_constants {
        return None;
    }
    match function.op(*result)? {
        Op::Const(value) => Some(Value::Number(*value)),
        Op::ConstBits(raw) => Some(Value::from_raw(function.output, *raw)),
        _ => None,
    }
}
