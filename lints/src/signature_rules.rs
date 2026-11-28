use rustc_data_structures::fx::FxHashMap;
use rustc_hir::def_id::DefId;
use rustc_middle::ty::Ty;
use rustc_middle::ty::TyCtxt;
use rustc_middle::ty::TyKind;
use rustc_span::kw;

use crate::anchors::Anchors;
use crate::anchors::adt_of;
use crate::anchors::in_trait_impl;
use crate::anchors::is_source_fn;
use crate::anchors::type_arg_of;
use crate::body_scan::markers;
use crate::collect::key;
use crate::lints::ALWAYS_CHANGED_FILTER;
use crate::lints::BORROWED_REBORROWABLE;
use crate::lints::ZST_QUERY;
use crate::lints::emit;

pub fn check(tcx: TyCtxt<'_>, anchors: &Anchors, always_changed: &FxHashMap<String, String>) {
    for owner in tcx.hir_body_owners() {
        let def = owner.to_def_id();
        if !is_source_fn(tcx, def) {
            continue;
        }
        let in_trait_impl = in_trait_impl(tcx, def);
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
            for component in changed_filters(anchors, *input) {
                let Some(writer) = always_changed.get(&key(tcx, component)) else {
                    continue;
                };
                let name = tcx.def_path_str(component);
                emit(tcx, ALWAYS_CHANGED_FILTER, hir_id, param.span, |diag| {
                    diag.primary_message(format!(
                        "`Changed<{name}>` matches every entity: system `{writer}` writes `{name}` on each tick"
                    ));
                    diag.help("the filter still visits every candidate; let the writer send the changed entities in a `Message`, or drop the filter");
                });
            }
            for marker in zst_fetches(anchors, *input) {
                let name = tcx.def_path_str(marker);
                emit(tcx, ZST_QUERY, hir_id, param.span, |diag| {
                    diag.primary_message(format!("the query fetches the marker `{name}` as data"));
                    diag.help(format!(
                        "filter with `With<{name}>`, or read presence with `Has<{name}>`"
                    ));
                });
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
                && let Some(reborrowable) = adt_of(*inner)
                && anchors.reborrowable.contains(&reborrowable)
                && !sig
                    .output()
                    .walk()
                    .any(|arg| arg.as_region() == Some(*region))
            {
                let name = tcx.def_path_str(reborrowable);
                emit(tcx, BORROWED_REBORROWABLE, hir_id, param.span, |diag| {
                    diag.primary_message(format!("the parameter takes `&mut {name}`"));
                    diag.help("take it by value; the caller passes `value.reborrow()`");
                });
            }
        }
    }
}

fn changed_filters(anchors: &Anchors, ty: Ty<'_>) -> Vec<DefId> {
    if anchors.query_data(ty).is_none() {
        return Vec::new();
    }
    ty.walk()
        .filter_map(|arg| arg.as_type())
        .filter_map(|ty| type_arg_of(ty, anchors.changed).and_then(adt_of))
        .collect()
}

fn zst_fetches(anchors: &Anchors, ty: Ty<'_>) -> Vec<DefId> {
    anchors.query_data(ty).map_or_else(Vec::new, fetched)
}

fn fetched(ty: Ty<'_>) -> Vec<DefId> {
    match ty.kind() {
        TyKind::Tuple(items) => items.iter().flat_map(fetched).collect(),
        TyKind::Ref(_, inner, _) => markers(*inner),
        _ => Vec::new(),
    }
}
