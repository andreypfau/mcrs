use std::collections::{HashMap, HashSet};

use mcrs_minecraft_entity_model::Json;
use mcrs_molang::{
    Accesses, CompileOptions, Expression, Milestone, QueryRegistry, Source, Value, VariableLayout,
    VariableSlot, WarningKind,
};

use crate::animation_diagnostic::{AnimationDiagnostic, DiagnosticKind};
use crate::entity_query::EntityQuery;

/// Compiles the Molang fields of one client entity into one variable layout and one query
/// registry.
#[derive(Debug)]
pub(crate) struct FieldCompiler {
    pub registry: QueryRegistry,
    pub layout: VariableLayout,
    pub diagnostics: Vec<AnimationDiagnostic>,
    /// The number of compiled fields that write each variable.
    writers: HashMap<VariableSlot, u32>,
    warned: HashSet<String>,
}

impl FieldCompiler {
    pub fn new() -> Self {
        Self {
            registry: EntityQuery::registry(),
            layout: VariableLayout::new(),
            diagnostics: Vec::new(),
            writers: HashMap::new(),
            warned: HashSet::new(),
        }
    }

    pub fn constant(value: f32) -> Expression {
        Expression::Constant(Value::Number(value))
    }

    /// Compiles a JSON value of a Molang field. A load error gives the constant `0.0`.
    pub fn compile(&mut self, json: &Json, context: &str) -> Expression {
        match json {
            Json::Number(value) => self.compile_source(&Source::Number(*value), context),
            Json::Bool(value) => self.compile_source(&Source::Bool(*value), context),
            Json::String(text) => self.compile_source(&Source::Text(text), context),
            Json::Object(object) => {
                let Some(inner) = object.get("expression") else {
                    return self.compile_source(&Source::Other, context);
                };
                let version = match object.get("version") {
                    None => None,
                    Some(Json::Number(version)) => Some(*version),
                    Some(_) => Some(f64::NAN),
                };
                let expression = match inner {
                    Json::Number(value) => Source::Number(*value),
                    Json::Bool(value) => Source::Bool(*value),
                    Json::String(text) => Source::Text(text),
                    _ => Source::Other,
                };
                let source = Source::Versioned {
                    expression: &expression,
                    version,
                };
                self.compile_source(&source, context)
            }
            Json::Null | Json::Array(_) => self.compile_source(&Source::Other, context),
        }
    }

    /// Compiles a script array: the entries joined with no text between them.
    pub fn compile_script(&mut self, entries: &[&str], context: &str) -> Expression {
        self.compile_source(&Source::ScriptArray(entries), context)
    }

    /// Whether a variable that `accesses` writes has more than one compiled field that writes it.
    pub fn has_other_writer(&self, accesses: &Accesses) -> bool {
        accesses
            .variables_written
            .iter()
            .any(|slot| self.writers.get(slot).copied().unwrap_or(0) > 1)
    }

    fn compile_source(&mut self, source: &Source<'_>, context: &str) -> Expression {
        let options = CompileOptions {
            milestone: Milestone::Two,
            ..CompileOptions::default()
        };
        match mcrs_molang::compile(source, &mut self.layout, &mut self.registry, &options) {
            Ok(compiled) => {
                for warning in &compiled.warnings {
                    let first = match warning.kind {
                        WarningKind::UnknownQuery | WarningKind::QueryOutOfContext => {
                            self.warned.insert(warning.name.clone())
                        }
                        _ => true,
                    };
                    if first {
                        self.diagnostics.push(AnimationDiagnostic::new(
                            DiagnosticKind::LoadWarning,
                            context,
                            format!("{:?} `{}`", warning.kind, warning.name),
                        ));
                    }
                }
                for slot in &compiled.expression.accesses().variables_written {
                    let count = self.writers.entry(*slot).or_insert(0);
                    *count = count.saturating_add(1);
                }
                compiled.expression
            }
            Err(errors) => {
                for error in errors {
                    self.diagnostics.push(AnimationDiagnostic::new(
                        DiagnosticKind::LoadError,
                        context,
                        error.to_string(),
                    ));
                }
                Self::constant(0.0)
            }
        }
    }
}
