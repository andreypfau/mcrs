use core::fmt;

/// The kind of a problem that the loader found and worked around.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum DiagnosticKind {
    /// A field with a wrong shape; the loader ignores it or uses its default.
    ContentError,
    /// A Molang load error; the expression is the constant `0.0`.
    LoadError,
    /// A Molang load warning, for example an unknown query; the expression runs.
    LoadWarning,
    /// A feature of a later milestone; the loader ignores it.
    Unsupported,
}

/// One problem of a file of the entity pack.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PackDiagnostic {
    pub kind: DiagnosticKind,
    /// Where the problem is, for example `animation.frog.walk/bones/body/rotation`.
    pub context: String,
    pub message: String,
}

impl PackDiagnostic {
    #[must_use]
    pub fn new(kind: DiagnosticKind, context: &str, message: impl Into<String>) -> Self {
        Self {
            kind,
            context: context.into(),
            message: message.into(),
        }
    }
}

impl fmt::Display for PackDiagnostic {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let kind = match self.kind {
            DiagnosticKind::ContentError => "content error",
            DiagnosticKind::LoadError => "load error",
            DiagnosticKind::LoadWarning => "load warning",
            DiagnosticKind::Unsupported => "unsupported",
        };
        write!(formatter, "{kind} in {}: {}", self.context, self.message)
    }
}
