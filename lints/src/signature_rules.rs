use rustc_errors::Diag;
use rustc_errors::DiagDecorator;
use rustc_hir::def::DefKind;
use rustc_middle::ty::Ty;
use rustc_middle::ty::TyCtxt;
use rustc_middle::ty::TyKind;
use rustc_span::kw;

use crate::anchors::Anchors;
use crate::anchors::adt_of;
use crate::body_scan::markers;
use crate::lints::BORROWED_REBORROWABLE;
use crate::lints::ZST_QUERY;

pub fn check(tcx: TyCtxt<'_>, anchors: &Anchors) {
    for owner in tcx.hir_body_owners() {
        let def = owner.to_def_id();
        if !matches!(tcx.def_kind(def), DefKind::Fn | DefKind::AssocFn) {
            continue;
        }
        if tcx.def_span(def).from_expansion() {
            continue;
        }
        let in_trait_impl = matches!(
            tcx.def_kind(tcx.parent(def)),
            DefKind::Impl { of_trait: true }
        );
        let sig = tcx
            .fn_sig(def)
            .instantiate_identity()
            .skip_normalization()
            .skip_binder();
        let body = tcx.hir_body_owned_by(owner);
        let hir_id = tcx.local_def_id_to_hir_id(owner);
        let names = tcx.fn_arg_idents(def);
        for (index, input) in sig.inputs().iter().enumerate() {
            let Some(param) = body.params.get(index) else {
                continue;
            };
            for marker in zst_fetches(anchors, *input) {
                let name = tcx.def_path_str(marker);
                tcx.emit_node_span_lint(
                    ZST_QUERY,
                    hir_id,
                    param.span,
                    DiagDecorator(|diag: &mut Diag<'_, ()>| {
                        diag.primary_message(format!(
                            "the query fetches the marker `{name}` as data"
                        ));
                        diag.help(format!(
                            "filter with `With<{name}>`, or read presence with `Has<{name}>`"
                        ));
                    }),
                );
            }
            let is_self = names
                .get(index)
                .copied()
                .flatten()
                .is_some_and(|ident| ident.name == kw::SelfLower);
            if in_trait_impl || is_self {
                continue;
            }
            if let TyKind::Ref(region, inner, mutability) = input.kind()
                && mutability.is_mut()
                && adt_of(*inner).is_some_and(|did| anchors.reborrowable.contains(&did))
                && !sig
                    .output()
                    .walk()
                    .any(|arg| arg.as_region() == Some(*region))
            {
                let name = tcx.def_path_str(adt_of(*inner).unwrap_or(def));
                tcx.emit_node_span_lint(
                    BORROWED_REBORROWABLE,
                    hir_id,
                    param.span,
                    DiagDecorator(|diag: &mut Diag<'_, ()>| {
                        diag.primary_message(format!("the parameter takes `&mut {name}`"));
                        diag.help("take it by value; the caller passes `value.reborrow()`");
                    }),
                );
            }
        }
    }
}

fn zst_fetches<'tcx>(anchors: &Anchors, ty: Ty<'tcx>) -> Vec<rustc_hir::def_id::DefId> {
    let TyKind::Adt(adt, args) = ty.kind() else {
        return Vec::new();
    };
    if adt.did() != anchors.query {
        return Vec::new();
    }
    let Some(data) = args.types().next() else {
        return Vec::new();
    };
    fetched(data)
}

fn fetched(ty: Ty<'_>) -> Vec<rustc_hir::def_id::DefId> {
    match ty.kind() {
        TyKind::Tuple(items) => items.iter().flat_map(fetched).collect(),
        TyKind::Ref(_, inner, _) => markers(*inner),
        _ => Vec::new(),
    }
}
