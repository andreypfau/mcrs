use std::collections::{HashMap, HashSet};

use mcrs_minecraft_entity_model::Json;
use mcrs_molang::{
    CompileOptions, Expression, Milestone, QueryRegistry, Source, Value, VariableLayout,
    WarningKind,
};

use crate::animation_diagnostic::{AnimationDiagnostic, DiagnosticKind};
use crate::dependencies::Dependencies;
use crate::entity_query::EntityQuery;

/// A compiled Molang field and the names it reads and writes.
#[derive(Clone, Debug)]
pub(crate) struct Field {
    pub expression: Expression,
    pub dependencies: Dependencies,
}

/// Compiles the Molang fields of one client entity into one variable layout and one query
/// registry.
#[derive(Debug)]
pub(crate) struct FieldCompiler {
    pub registry: QueryRegistry,
    pub layout: VariableLayout,
    pub diagnostics: Vec<AnimationDiagnostic>,
    /// The number of compiled fields that write each variable.
    writers: HashMap<Box<str>, u32>,
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

    pub fn constant(value: f32) -> Field {
        Field {
            expression: Expression::Constant(Value::Number(value)),
            dependencies: Dependencies::default(),
        }
    }

    /// Compiles a JSON value of a Molang field. A load error gives the constant `0.0`.
    pub fn compile(&mut self, json: &Json, context: &str) -> Field {
        match json {
            Json::Number(value) => self.compile_source(&Source::Number(*value), None, context),
            Json::Bool(value) => self.compile_source(&Source::Bool(*value), None, context),
            Json::String(text) => self.compile_source(&Source::Text(text), Some(text), context),
            Json::Object(object) => {
                let Some(inner) = object.get("expression") else {
                    return self.compile_source(&Source::Other, None, context);
                };
                let version = match object.get("version") {
                    None => None,
                    Some(Json::Number(version)) => Some(*version),
                    Some(_) => Some(f64::NAN),
                };
                let (expression, text) = match inner {
                    Json::Number(value) => (Source::Number(*value), None),
                    Json::Bool(value) => (Source::Bool(*value), None),
                    Json::String(text) => (Source::Text(text), Some(text.as_str())),
                    _ => (Source::Other, None),
                };
                let source = Source::Versioned {
                    expression: &expression,
                    version,
                };
                self.compile_source(&source, text, context)
            }
            Json::Null | Json::Array(_) => self.compile_source(&Source::Other, None, context),
        }
    }

    /// Compiles a script array: the entries joined with no text between them.
    pub fn compile_script(&mut self, entries: &[&str], context: &str) -> Field {
        let text = entries.concat();
        self.compile_source(&Source::ScriptArray(entries), Some(&text), context)
    }

    /// Whether a compiled field other than one with `dependencies` writes one of its variables.
    pub fn has_other_writer(&self, dependencies: &Dependencies) -> bool {
        dependencies
            .writes
            .iter()
            .any(|name| self.writers.get(name).copied().unwrap_or(0) > 1)
    }

    fn compile_source(&mut self, source: &Source<'_>, text: Option<&str>, context: &str) -> Field {
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
                let dependencies = text
                    .and_then(|text| mcrs_molang::parse(text).ok())
                    .map(|syntax| Dependencies::of(&syntax))
                    .unwrap_or_default();
                for name in &dependencies.writes {
                    let count = self.writers.entry(name.clone()).or_insert(0);
                    *count = count.saturating_add(1);
                }
                Field {
                    expression: compiled.expression,
                    dependencies,
                }
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
