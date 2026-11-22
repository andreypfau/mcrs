use rustc_data_structures::fx::FxHashMap;
use rustc_errors::Diag;
use rustc_errors::DiagDecorator;
use rustc_hir::BinOpKind;
use rustc_hir::BodyId;
use rustc_hir::Expr;
use rustc_hir::ExprKind;
use rustc_hir::UnOp;
use rustc_hir::def::DefKind;
use rustc_hir::def_id::DefId;
use rustc_hir::def_id::LocalDefId;
use rustc_hir::intravisit::Visitor;
use rustc_hir::intravisit::walk_expr;
use rustc_infer::infer::TyCtxtInferExt;
use rustc_middle::hir::nested_filter;
use rustc_middle::ty::GenericArgsRef;
use rustc_middle::ty::Ty;
use rustc_middle::ty::TyCtxt;
use rustc_middle::ty::TyKind;
use rustc_middle::ty::TypeckResults;
use rustc_middle::ty::TypingEnv;
use rustc_span::Span;
use rustc_trait_selection::infer::InferCtxtExt;

use crate::anchors::Anchors;
use crate::anchors::adt_of;
use crate::lints::INSERT_MESSAGE_RESOURCE;
use crate::lints::MISSED_SET_IF_NEQ;

#[derive(Default)]
pub struct BodyFacts {
    pub blocking: Vec<(Span, DefId)>,
    pub allocs: Vec<Span>,
    pub resource_inserts: Vec<Span>,
    pub marker_inserts: Vec<(DefId, Span)>,
    pub marker_removes: Vec<(DefId, Span)>,
    pub closures: Vec<LocalDefId>,
    pub calls: Vec<LocalDefId>,
}

pub type BodyMap = FxHashMap<LocalDefId, BodyFacts>;

pub fn scan(tcx: TyCtxt<'_>, anchors: &Anchors) -> BodyMap {
    let mut bodies = BodyMap::default();
    for owner in tcx.hir_body_owners() {
        if tcx.is_typeck_child(owner.to_def_id()) {
            continue;
        }
        let mut scanner = Scanner {
            tcx,
            typeck: tcx.typeck(owner),
            root: owner,
            owners: vec![owner],
            conditional: 0,
            compared: 0,
            anchors,
            bodies: &mut bodies,
        };
        scanner.visit_body(tcx.hir_body_owned_by(owner));
    }
    bodies
}

struct Scanner<'a, 'tcx> {
    tcx: TyCtxt<'tcx>,
    typeck: &'tcx TypeckResults<'tcx>,
    root: LocalDefId,
    owners: Vec<LocalDefId>,
    conditional: usize,
    compared: usize,
    anchors: &'a Anchors,
    bodies: &'a mut BodyMap,
}

impl<'tcx> Visitor<'tcx> for Scanner<'_, 'tcx> {
    type NestedFilter = nested_filter::OnlyBodies;

    fn maybe_tcx(&mut self) -> TyCtxt<'tcx> {
        self.tcx
    }

    fn visit_nested_body(&mut self, id: BodyId) {
        let owner = self.tcx.hir_body_owner_def_id(id);
        if !self.tcx.is_typeck_child(owner.to_def_id()) {
            return;
        }
        if self.tcx.is_closure_like(owner.to_def_id()) {
            self.facts().closures.push(owner);
        }
        self.owners.push(owner);
        self.visit_body(self.tcx.hir_body(id));
        self.owners.pop();
    }

    fn visit_expr(&mut self, expr: &'tcx Expr<'tcx>) {
        match expr.kind {
            ExprKind::If(condition, ..) => {
                let compares = compares(condition);
                self.conditional += 1;
                self.compared += usize::from(compares);
                walk_expr(self, expr);
                self.compared -= usize::from(compares);
                self.conditional -= 1;
            }
            ExprKind::Match(..) | ExprKind::Loop(..) => {
                self.conditional += 1;
                walk_expr(self, expr);
                self.conditional -= 1;
            }
            ExprKind::Assign(place, ..) => {
                self.check_assign(expr, place);
                walk_expr(self, expr);
            }
            _ => {
                self.inspect_call(expr);
                walk_expr(self, expr);
            }
        }
    }
}

impl<'tcx> Scanner<'_, 'tcx> {
    fn facts(&mut self) -> &mut BodyFacts {
        let owner = self.owners.last().copied().unwrap_or(self.root);
        self.bodies.entry(owner).or_default()
    }

    fn inspect_call(&mut self, expr: &'tcx Expr<'tcx>) {
        let (callee, generics, args): (DefId, GenericArgsRef<'tcx>, Vec<&'tcx Expr<'tcx>>) =
            match expr.kind {
                ExprKind::MethodCall(_, receiver, args, _) => {
                    let Some(callee) = self.typeck.type_dependent_def_id(expr.hir_id) else {
                        return;
                    };
                    (
                        callee,
                        self.typeck.node_args(expr.hir_id),
                        std::iter::once(receiver).chain(args.iter()).collect(),
                    )
                }
                ExprKind::Call(func, args) => match self.typeck.node_type_opt(func.hir_id) {
                    Some(ty) => match *ty.kind() {
                        TyKind::FnDef(callee, generics) => {
                            (callee, generics.skip_binder(), args.iter().collect())
                        }
                        _ => return,
                    },
                    None => return,
                },
                _ => return,
            };
        let tcx = self.tcx;
        let anchors = self.anchors;
        if anchors.is_blocking(tcx, callee) {
            self.facts().blocking.push((expr.span, callee));
        }
        if let Some(local) = callee.as_local()
            && matches!(tcx.def_kind(callee), DefKind::Fn | DefKind::AssocFn)
        {
            self.facts().calls.push(local);
        }
        if !anchors.moves.contains(&callee) && anchors.is_heap(self.typeck.expr_ty(expr)) {
            self.facts().allocs.push(expr.span);
        }
        let Some(host) = anchors.method_host(tcx, callee) else {
            return;
        };
        let name = tcx.item_name(callee);
        match name.as_str() {
            "insert_resource" | "init_resource" if anchors.resource_hosts.contains(&host) => {
                let resource = generics
                    .types()
                    .chain(args.get(1).map(|arg| self.typeck.expr_ty(arg)))
                    .any(|ty| adt_of(ty) == Some(anchors.messages));
                if resource {
                    tcx.emit_node_span_lint(
                        INSERT_MESSAGE_RESOURCE,
                        expr.hir_id,
                        expr.span,
                        DiagDecorator(|diag: &mut Diag<'_, ()>| {
                            diag.primary_message("`Messages<T>` is inserted as a plain resource");
                            diag.help("use `add_message::<T>()`, which also clears old messages");
                        }),
                    );
                }
                if name.as_str() == "insert_resource" && self.conditional == 0 {
                    self.facts().resource_inserts.push(expr.span);
                }
            }
            "insert" | "try_insert" | "insert_if_new" | "try_insert_if_new"
                if anchors.entity_hosts.contains(&host) =>
            {
                if let Some(bundle) = args.get(1) {
                    let span = expr.span;
                    let markers = markers(self.typeck.expr_ty(bundle));
                    self.facts()
                        .marker_inserts
                        .extend(markers.into_iter().map(|marker| (marker, span)));
                }
            }
            "remove" | "try_remove" if anchors.entity_hosts.contains(&host) => {
                let span = expr.span;
                let markers: Vec<DefId> = generics.types().flat_map(markers).collect();
                self.facts()
                    .marker_removes
                    .extend(markers.into_iter().map(|marker| (marker, span)));
            }
            _ => {}
        }
    }

    fn check_assign(&mut self, expr: &'tcx Expr<'tcx>, place: &'tcx Expr<'tcx>) {
        if self.compared > 0 || !self.through_change_tracked(place) {
            return;
        }
        let ty = self.typeck.expr_ty(place);
        if !self.is_partial_eq(ty) {
            return;
        }
        self.tcx.emit_node_span_lint(
            MISSED_SET_IF_NEQ,
            expr.hir_id,
            expr.span,
            DiagDecorator(|diag: &mut Diag<'_, ()>| {
                diag.primary_message("a write through `DerefMut` marks the value changed even when it is equal");
                diag.help("use `set_if_neq`, or `map_unchanged(|v| &mut v.field).set_if_neq(..)` for one field");
            }),
        );
    }

    fn through_change_tracked(&self, place: &'tcx Expr<'tcx>) -> bool {
        let mut current = place;
        loop {
            let base = match current.kind {
                ExprKind::Field(base, _) | ExprKind::Index(base, ..) => base,
                ExprKind::Unary(UnOp::Deref, base) => base,
                _ => return false,
            };
            if adt_of(self.typeck.expr_ty(base))
                .is_some_and(|did| self.anchors.change_tracked.contains(&did))
            {
                return true;
            }
            current = base;
        }
    }

    fn is_partial_eq(&self, ty: Ty<'tcx>) -> bool {
        let Some(partial_eq) = self.anchors.partial_eq else {
            return false;
        };
        let env = TypingEnv::post_analysis(self.tcx, self.root);
        let (infcx, param_env) = self.tcx.infer_ctxt().build_with_typing_env(env);
        infcx
            .type_implements_trait(partial_eq, [ty, ty], param_env)
            .must_apply_modulo_regions()
    }
}

fn compares(expr: &Expr<'_>) -> bool {
    match expr.kind {
        ExprKind::Binary(op, left, right) => {
            matches!(op.node, BinOpKind::Eq | BinOpKind::Ne) || compares(left) || compares(right)
        }
        ExprKind::Unary(_, inner) | ExprKind::DropTemps(inner) => compares(inner),
        ExprKind::MethodCall(segment, ..) => matches!(segment.ident.name.as_str(), "eq" | "ne"),
        _ => false,
    }
}

pub fn markers(ty: Ty<'_>) -> Vec<DefId> {
    match ty.kind() {
        TyKind::Tuple(items) => items.iter().flat_map(markers).collect(),
        TyKind::Adt(adt, _) if adt.is_struct() && adt.non_enum_variant().fields.is_empty() => {
            vec![adt.did()]
        }
        _ => Vec::new(),
    }
}
