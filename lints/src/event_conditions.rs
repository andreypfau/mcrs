use rustc_data_structures::fx::FxHashSet;
use rustc_hir::def::DefKind;
use rustc_hir::def_id::DefId;
use rustc_hir::def_id::LocalDefId;
use rustc_middle::ty::TyCtxt;
use rustc_span::Symbol;

use crate::lints::INVALID_EVENT_CONDITION;
use crate::lints::emit;

#[derive(Default)]
pub struct EventConditions {
    pub valid: Vec<LocalDefId>,
    misuse: Vec<(LocalDefId, Misuse)>,
}

#[derive(Clone, Copy)]
enum Misuse {
    NotFunction,
    NotBool,
}

pub fn scan(tcx: TyCtxt<'_>) -> EventConditions {
    let mut found = EventConditions::default();
    for def in tcx.hir_crate_items(()).definitions() {
        if !is_marked(tcx, def) {
            continue;
        }
        match tcx.def_kind(def) {
            DefKind::Fn | DefKind::AssocFn => {
                let returns_bool = tcx
                    .fn_sig(def)
                    .instantiate_identity()
                    .skip_normalization()
                    .skip_binder()
                    .output()
                    .is_bool();
                if returns_bool {
                    found.valid.push(def);
                } else {
                    found.misuse.push((def, Misuse::NotBool));
                }
            }
            _ => found.misuse.push((def, Misuse::NotFunction)),
        }
    }
    found
}

impl EventConditions {
    pub fn report(&self, tcx: TyCtxt<'_>, used: &FxHashSet<DefId>) {
        let exported = tcx.effective_visibilities(());
        let unused = self
            .valid
            .iter()
            .filter(|def| !used.contains(&def.to_def_id()) && !exported.is_exported(**def));
        let misuse = self
            .misuse
            .iter()
            .map(|(def, misuse)| (*def, message(*misuse)))
            .chain(unused.map(|def| (*def, "the function is never used as a run condition")));
        for (def, message) in misuse {
            let name = tcx.def_path_str(def.to_def_id());
            emit(
                tcx,
                INVALID_EVENT_CONDITION,
                tcx.local_def_id_to_hir_id(def),
                tcx.def_span(def),
                |diag| {
                    diag.primary_message(format!(
                        "`{name}` is marked `mcrs::event_condition`: {message}"
                    ));
                    diag.help("mark only a function that returns `bool` and gates a system");
                },
            );
        }
    }
}

fn message(misuse: Misuse) -> &'static str {
    match misuse {
        Misuse::NotFunction => "the item is not a function",
        Misuse::NotBool => "the function does not return `bool`",
    }
}

fn is_marked(tcx: TyCtxt<'_>, def: LocalDefId) -> bool {
    let tool = Symbol::intern("mcrs");
    let name = Symbol::intern("event_condition");
    tcx.hir_attrs(tcx.local_def_id_to_hir_id(def))
        .iter()
        .any(|attr| attr.path_matches(&[tool, name]))
}
