use std::collections::{HashMap, HashSet};

use mcrs_molang::{
    Accesses, CompileOptions, Expression, Milestone, QueryRegistry, ResourceSet, Source, Value,
    VariableLayout, VariableSlot, WarningKind,
};

use crate::json::Json;
use crate::pack_diagnostic::{DiagnosticKind, PackDiagnostic};

#[derive(Debug)]
pub struct FieldCompiler {
    pub registry: QueryRegistry,
    pub layout: VariableLayout,
    pub diagnostics: Vec<PackDiagnostic>,
    writers: HashMap<VariableSlot, u32>,
    warned: HashSet<String>,
}

impl FieldCompiler {
    #[must_use]
    pub fn new(registry: QueryRegistry) -> Self {
        Self {
            registry,
            layout: VariableLayout::new(),
            diagnostics: Vec::new(),
            writers: HashMap::new(),
            warned: HashSet::new(),
        }
    }

    #[must_use]
    pub fn constant(value: f32) -> Expression {
        Expression::Constant(Value::Number(value))
    }

    pub fn report(&mut self, kind: DiagnosticKind, context: &str, message: impl Into<String>) {
        self.diagnostics
            .push(PackDiagnostic::new(kind, context, message));
    }

    pub fn compile(&mut self, json: &Json, context: &str) -> Expression {
        self.try_compile(json, context, None)
            .unwrap_or_else(|| Self::constant(0.0))
    }

    pub fn try_compile(
        &mut self,
        json: &Json,
        context: &str,
        resources: Option<&ResourceSet>,
    ) -> Option<Expression> {
        let Json::Object(object) = json else {
            return self.compile_source(&source(json), context, resources);
        };
        let Some(inner) = object.get("expression") else {
            return self.compile_source(&Source::Other, context, resources);
        };
        let version = match object.get("version") {
            None => None,
            Some(Json::Number(version)) => Some(*version),
            Some(_) => Some(f64::NAN),
        };
        let versioned = Source::Versioned {
            expression: &source(inner),
            version,
        };
        self.compile_source(&versioned, context, resources)
    }

    pub fn compile_script(&mut self, entries: &[&str], context: &str) -> Expression {
        self.compile_source(&Source::ScriptArray(entries), context, None)
            .unwrap_or_else(|| Self::constant(0.0))
    }

    #[must_use]
    pub fn has_other_writer(&self, accesses: &Accesses) -> bool {
        accesses
            .variables_written
            .iter()
            .any(|slot| self.writers.get(slot).copied().unwrap_or(0) > 1)
    }

    fn compile_source(
        &mut self,
        source: &Source<'_>,
        context: &str,
        resources: Option<&ResourceSet>,
    ) -> Option<Expression> {
        let options = CompileOptions {
            milestone: Milestone::Two,
            resources,
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
                        let message = format!("{:?} `{}`", warning.kind, warning.name);
                        self.report(DiagnosticKind::LoadWarning, context, message);
                    }
                }
                for slot in &compiled.expression.accesses().variables_written {
                    let count = self.writers.entry(*slot).or_insert(0);
                    *count = count.saturating_add(1);
                }
                Some(compiled.expression)
            }
            Err(errors) => {
                for error in errors {
                    self.report(DiagnosticKind::LoadError, context, error.to_string());
                }
                None
            }
        }
    }
}

fn source(json: &Json) -> Source<'_> {
    match json {
        Json::Number(value) => Source::Number(*value),
        Json::Bool(value) => Source::Bool(*value),
        Json::String(text) => Source::Text(text),
        Json::Null | Json::Array(_) | Json::Object(_) => Source::Other,
    }
}
