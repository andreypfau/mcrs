use core::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum DiagnosticKind {
    ContentError,
    LoadError,
    LoadWarning,
    Unsupported,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PackDiagnostic {
    pub kind: DiagnosticKind,
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
