use rustc_errors::Diag;
use rustc_errors::DiagDecorator;
use rustc_hir::def::DefKind;
use rustc_hir::def_id::DefId;
use rustc_infer::infer::TyCtxtInferExt;
use rustc_middle::ty::Ty;
use rustc_middle::ty::TyCtxt;
use rustc_middle::ty::TypingEnv;
use rustc_trait_selection::infer::InferCtxtExt;

use crate::lints::TOO_MANY_ARGUMENTS;

const MAX_ARGUMENTS: usize = 7;

pub fn check<'tcx>(tcx: TyCtxt<'tcx>, system_param: Option<DefId>) {
    for owner in tcx.hir_crate_items(()).definitions() {
        let def = owner.to_def_id();
        if !matches!(tcx.def_kind(def), DefKind::Fn | DefKind::AssocFn)
            || tcx.def_span(def).from_expansion()
            || matches!(
                tcx.def_kind(tcx.parent(def)),
                DefKind::Impl { of_trait: true }
            )
        {
            continue;
        }
        let sig = tcx.liberate_late_bound_regions(
            def,
            tcx.fn_sig(def).instantiate_identity().skip_normalization(),
        );
        if sig.inputs().len() <= MAX_ARGUMENTS {
            continue;
        }
        let env = TypingEnv::post_analysis(tcx, owner);
        let (infcx, param_env) = tcx.infer_ctxt().build_with_typing_env(env);
        let is_param = |ty: Ty<'tcx>| {
            system_param.is_some_and(|param| {
                infcx
                    .type_implements_trait(param, [ty], param_env)
                    .must_apply_modulo_regions()
            })
        };
        let count = sig.inputs().iter().filter(|ty| !is_param(**ty)).count();
        if count <= MAX_ARGUMENTS {
            continue;
        }
        let span = tcx.def_ident_span(def).unwrap_or_else(|| tcx.def_span(def));
        tcx.emit_node_span_lint(
            TOO_MANY_ARGUMENTS,
            tcx.local_def_id_to_hir_id(owner),
            span,
            DiagDecorator(|diag: &mut Diag<'_, ()>| {
                diag.primary_message(format!(
                    "the function has {count} parameters that are not system parameters (at most {MAX_ARGUMENTS})"
                ));
                diag.help("group the values that travel together in a struct");
            }),
        );
    }
}
