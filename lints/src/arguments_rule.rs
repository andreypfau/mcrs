use rustc_hir::def_id::DefId;
use rustc_middle::ty::TyCtxt;

use crate::anchors::implements;
use crate::anchors::in_trait_impl;
use crate::anchors::is_source_fn;
use crate::lints::TOO_MANY_ARGUMENTS;
use crate::lints::emit;

const MAX_ARGUMENTS: usize = 7;

pub fn check(tcx: TyCtxt<'_>, system_param: Option<DefId>) {
    for owner in tcx.hir_crate_items(()).definitions() {
        let def = owner.to_def_id();
        if !is_source_fn(tcx, def) || in_trait_impl(tcx, def) {
            continue;
        }
        let sig = tcx.liberate_late_bound_regions(
            def,
            tcx.fn_sig(def).instantiate_identity().skip_normalization(),
        );
        if sig.inputs().len() <= MAX_ARGUMENTS {
            continue;
        }
        let count = sig
            .inputs()
            .iter()
            .filter(|ty| !system_param.is_some_and(|param| implements(tcx, owner, param, [**ty])))
            .count();
        if count <= MAX_ARGUMENTS {
            continue;
        }
        let span = tcx.def_ident_span(def).unwrap_or_else(|| tcx.def_span(def));
        let hir_id = tcx.local_def_id_to_hir_id(owner);
        emit(tcx, TOO_MANY_ARGUMENTS, hir_id, span, |diag| {
            diag.primary_message(format!(
                "the function has {count} parameters that are not system parameters (at most {MAX_ARGUMENTS})"
            ));
            diag.help("group the values that travel together in a struct");
        });
    }
}
