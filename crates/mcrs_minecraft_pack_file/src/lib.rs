//! The files of an entity pack as the loaders of geometry, animation and render controllers
//! read them: lenient JSON, the client entity file, the load diagnostics and the compile of
//! Molang fields.

// std: serde_json

mod client_entity_file;
mod field_compiler;
mod json;
mod pack_diagnostic;
mod pack_file_error;
mod scripts;

pub use client_entity_file::{AnimationName, ClientEntityFile, ControllerEntry};
pub use field_compiler::FieldCompiler;
pub use json::{Json, JsonObject, SyntaxError};
pub use pack_diagnostic::{DiagnosticKind, PackDiagnostic};
pub use pack_file_error::PackFileError;
pub use scripts::{AnimateItem, SCALE_FIELDS, Scripts};
