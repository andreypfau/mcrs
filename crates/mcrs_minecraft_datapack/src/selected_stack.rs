use crate::{PackDiagnostic, PackStack};

pub struct SelectedStack {
    pub stack: PackStack,
    pub diagnostics: Vec<PackDiagnostic>,
}
