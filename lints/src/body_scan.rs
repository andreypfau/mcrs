use rustc_ast::LitKind;
use rustc_data_structures::fx::FxHashMap;
use rustc_hir::BinOpKind;
use rustc_hir::Block;
use rustc_hir::BodyId;
use rustc_hir::Expr;
use rustc_hir::ExprKind;
use rustc_hir::HirId;
use rustc_hir::LetStmt;
use rustc_hir::MatchSource;
use rustc_hir::Mutability;
use rustc_hir::PatKind;
use rustc_hir::QPath;
use rustc_hir::Stmt;
use rustc_hir::StmtKind;
use rustc_hir::UnOp;
use rustc_hir::def::DefKind;
use rustc_hir::def::Res;
use rustc_hir::def_id::DefId;
use rustc_hir::def_id::LocalDefId;
use rustc_hir::intravisit::Visitor;
use rustc_hir::intravisit::walk_expr;
use rustc_hir::intravisit::walk_stmt;
use rustc_middle::hir::nested_filter;
use rustc_middle::ty::GenericArgsRef;
use rustc_middle::ty::Ty;
use rustc_middle::ty::TyCtxt;
use rustc_middle::ty::TyKind;
use rustc_middle::ty::TypeckResults;
use rustc_middle::ty::adjustment::Adjust;
use rustc_middle::ty::adjustment::DerefAdjustKind;
use rustc_span::Span;

use crate::anchors::Anchors;
use crate::anchors::adt_of;
use crate::anchors::implements;
use crate::anchors::in_cfg_test;
use crate::anchors::type_arg_of;
use crate::collect::Call;
use crate::collect::call;
use crate::lints::INSERT_MESSAGE_RESOURCE;
use crate::lints::MISSED_SET_IF_NEQ;
use crate::lints::UNIT_IN_BUNDLE;
use crate::lints::emit;

#[derive(Default)]
pub struct BodyFacts {
    pub blocking: Vec<(Span, DefId)>,
    pub allocs: Vec<Span>,
    pub resource_inserts: Vec<Span>,
    pub marker_inserts: Vec<(DefId, Span)>,
    pub marker_removes: Vec<(DefId, Span)>,
    pub closures: Vec<LocalDefId>,
    pub calls: Vec<LocalDefId>,
    pub writes: Vec<DefId>,
}

pub type BodyMap = FxHashMap<LocalDefId, BodyFacts>;

pub fn scan(tcx: TyCtxt<'_>, anchors: &Anchors, test: bool) -> BodyMap {
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
            guarded: 0,
            compared: 0,
            finished: 0,
            detached: 0,
            event: 0,
            moved: Vec::new(),
            pending: FxHashMap::default(),
            test,
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
    guarded: usize,
    compared: usize,
    finished: usize,
    detached: usize,
    event: usize,
    moved: Vec<(LocalDefId, String)>,
    pending: FxHashMap<HirId, (LocalDefId, Span)>,
    test: bool,
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
        if self.detached == 0
            && self.tcx.is_closure_like(owner.to_def_id())
            && self.tcx.coroutine_kind(owner.to_def_id()).is_none()
        {
            self.facts().closures.push(owner);
        }
        self.owners.push(owner);
        self.visit_body(self.tcx.hir_body(id));
        self.owners.pop();
    }

    fn visit_stmt(&mut self, stmt: &'tcx Stmt<'tcx>) {
        if let StmtKind::Let(local) = stmt.kind {
            self.track_creation(local);
        }
        walk_stmt(self, stmt);
    }

    fn visit_block(&mut self, block: &'tcx Block<'tcx>) {
        let (mut compared, mut finished, mut event) = (0, 0, 0);
        for stmt in block.stmts {
            self.visit_stmt(stmt);
            let Some(condition) = early_exit(stmt) else {
                continue;
            };
            if compares(condition) {
                compared += 1;
                self.conditional += 1;
                self.compared += 1;
            }
            if calls_is_finished(condition) {
                finished += 1;
                self.finished += 1;
            }
            if self.mentions_event(condition) || self.change_guard(condition, Polarity::Unchanged) {
                event += 1;
                self.event += 1;
            }
        }
        if let Some(expr) = block.expr {
            self.visit_expr(expr);
        }
        self.conditional -= compared;
        self.compared -= compared;
        self.finished -= finished;
        self.event -= event;
    }

    fn visit_expr(&mut self, expr: &'tcx Expr<'tcx>) {
        self.record_write(expr);
        match expr.kind {
            ExprKind::If(condition, ..) => {
                let compares = compares(condition);
                let finished = calls_is_finished(condition);
                let event = usize::from(
                    self.mentions_event(condition)
                        || self.change_guard(condition, Polarity::Changed),
                );
                self.event += event;
                self.conditional += 1;
                self.guarded += 1;
                self.compared += usize::from(compares);
                self.finished += usize::from(finished);
                walk_expr(self, expr);
                self.finished -= usize::from(finished);
                self.compared -= usize::from(compares);
                self.guarded -= 1;
                self.conditional -= 1;
                self.event -= event;
            }
            ExprKind::Match(head, _, source) => {
                let guards = usize::from(source == MatchSource::Normal);
                let event = usize::from(self.mentions_event(head));
                self.event += event;
                self.conditional += 1;
                self.guarded += guards;
                walk_expr(self, expr);
                self.guarded -= guards;
                self.conditional -= 1;
                self.event -= event;
            }
            ExprKind::Loop(..) => {
                self.conditional += 1;
                walk_expr(self, expr);
                self.conditional -= 1;
            }
            ExprKind::Assign(place, value, _) => {
                self.forget(value);
                if !self.negates(place, value)
                    && !self.counts(place, value)
                    && !self.was_moved(place)
                {
                    self.check_assign(expr, place);
                }
                walk_expr(self, expr);
            }
            ExprKind::MethodCall(..)
                if self
                    .typeck
                    .type_dependent_def_id(expr.hir_id)
                    .is_some_and(|callee| self.anchors.is_observer_call(self.tcx, callee)) =>
            {
                self.inspect_call(expr);
                self.detached += 1;
                walk_expr(self, expr);
                self.detached -= 1;
            }
            ExprKind::Ret(Some(value)) => {
                self.forget(value);
                walk_expr(self, expr);
            }
            ExprKind::Struct(_, fields, _) => {
                for field in fields {
                    self.forget(field.expr);
                }
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
        let owner = self.owner();
        self.bodies.entry(owner).or_default()
    }

    fn record_write(&mut self, expr: &'tcx Expr<'tcx>) {
        if self.guarded > 0 {
            return;
        }
        let mut current = self.typeck.expr_ty(expr);
        let mut written = None;
        if let ExprKind::Unary(UnOp::Deref, inner) = expr.kind
            && self
                .typeck
                .type_dependent_def_id(expr.hir_id)
                .is_some_and(|method| self.tcx.item_name(method).as_str() == "deref_mut")
        {
            written = self.mut_component(self.typeck.expr_ty(inner));
        }
        for adjustment in self.typeck.expr_adjustments(expr) {
            if let Adjust::Deref(DerefAdjustKind::Overloaded(deref)) = adjustment.kind
                && deref.mutbl.is_mut()
            {
                written = written.or_else(|| self.mut_component(current));
            }
            current = adjustment.target;
        }
        if let Some(component) = written {
            self.facts().writes.push(component);
        }
    }

    fn mut_component(&self, ty: Ty<'tcx>) -> Option<DefId> {
        type_arg_of(ty.peel_refs(), self.anchors.mut_ref).and_then(adt_of)
    }

    fn check_unit_in_bundle(
        &self,
        expr: &'tcx Expr<'tcx>,
        callee: DefId,
        generics: GenericArgsRef<'tcx>,
    ) {
        if !generics.types().any(contains_unit) {
            return;
        }
        let bundled = self
            .tcx
            .clauses_of(callee)
            .clauses
            .iter()
            .filter_map(|(clause, _)| clause.as_trait_clause())
            .filter(|clause| clause.skip_binder().def_id() == self.anchors.bundle)
            .filter_map(|clause| match clause.skip_binder().self_ty().kind() {
                TyKind::Param(param) => Some(param.index as usize),
                _ => None,
            })
            .any(|index| {
                generics
                    .get(index)
                    .and_then(|arg| arg.as_type())
                    .is_some_and(contains_unit)
            });
        if bundled {
            emit(self.tcx, UNIT_IN_BUNDLE, expr.hir_id, expr.span, |diag| {
                diag.primary_message("the bundle contains `()`, which inserts nothing");
                diag.help(
                    "remove the `()`; spawn an entity with no components with `spawn_empty()`",
                );
            });
        }
    }

    fn track_creation(&mut self, local: &'tcx LetStmt<'tcx>) {
        if self.event > 0 || self.detached > 0 {
            return;
        }
        let (Some(init), PatKind::Binding(_, binding, _, None)) = (local.init, local.pat.kind)
        else {
            return;
        };
        let Some(Call { callee, args, .. }) = call(self.typeck, init) else {
            return;
        };
        if args.is_empty()
            && matches!(self.tcx.item_name(callee).as_str(), "new" | "default")
            && self.anchors.is_heap(self.typeck.expr_ty(init))
        {
            let owner = self.owner();
            self.pending.insert(binding, (owner, init.span));
        }
    }

    fn pending_binding(&self, expr: &Expr<'_>) -> Option<HirId> {
        let ExprKind::Path(QPath::Resolved(None, path)) = expr.kind else {
            return None;
        };
        match path.res {
            Res::Local(binding) if self.pending.contains_key(&binding) => Some(binding),
            _ => None,
        }
    }

    fn forget(&mut self, expr: &Expr<'_>) {
        if let Some(binding) = self.pending_binding(expr) {
            self.pending.remove(&binding);
        }
    }

    fn grow(&mut self, expr: &Expr<'_>) {
        let Some(binding) = self.pending_binding(expr) else {
            return;
        };
        if let Some((owner, span)) = self.pending.remove(&binding) {
            self.bodies.entry(owner).or_default().allocs.push(span);
        }
    }

    fn track_growth(&mut self, expr: &Expr<'_>, callee: DefId, args: &[&'tcx Expr<'tcx>]) {
        if self.pending.is_empty() {
            return;
        }
        let grows = matches!(
            self.tcx.item_name(callee).as_str(),
            "push"
                | "push_back"
                | "push_front"
                | "extend"
                | "extend_from_slice"
                | "insert"
                | "push_str"
                | "append"
                | "resize"
                | "reserve"
        );
        if let Some(receiver) = args.first().filter(|_| grows) {
            self.grow(receiver);
        }
        let receivers = usize::from(matches!(expr.kind, ExprKind::MethodCall(..)));
        for arg in args.iter().skip(receivers) {
            match arg.kind {
                ExprKind::AddrOf(_, Mutability::Mut, place) => self.grow(place),
                _ => self.forget(arg),
            }
        }
    }

    fn inspect_call(&mut self, expr: &'tcx Expr<'tcx>) {
        let Some(Call {
            callee,
            generics,
            args,
        }) = call(self.typeck, expr)
        else {
            return;
        };
        let tcx = self.tcx;
        let anchors = self.anchors;
        self.check_unit_in_bundle(expr, callee, generics);
        self.track_growth(expr, callee, &args);
        if anchors.moves.contains(&callee)
            && let Some(ExprKind::AddrOf(_, _, place)) = args.first().map(|arg| arg.kind)
            && let Some(root) = self.root(place)
        {
            let owner = self.owner();
            self.moved.push((owner, root));
        }
        if anchors.is_blocking(tcx, callee)
            && !(self.finished > 0 && tcx.item_name(callee).as_str() == "join")
        {
            self.facts().blocking.push((expr.span, callee));
        }
        if let Some(local) = callee.as_local()
            && matches!(tcx.def_kind(callee), DefKind::Fn | DefKind::AssocFn)
        {
            self.facts().calls.push(local);
        }
        let empty = args.is_empty() && matches!(tcx.item_name(callee).as_str(), "new" | "default");
        if self.event == 0
            && !empty
            && !anchors.moves.contains(&callee)
            && anchors.is_heap(self.typeck.expr_ty(expr))
        {
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
                    emit(
                        tcx,
                        INSERT_MESSAGE_RESOURCE,
                        expr.hir_id,
                        expr.span,
                        |diag| {
                            diag.primary_message("`Messages<T>` is inserted as a plain resource");
                            diag.help("use `add_message::<T>()`, which also clears old messages");
                        },
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

    fn mentions_event(&self, expr: &'tcx Expr<'tcx>) -> bool {
        let here = self.typeck.node_type_opt(expr.hir_id).is_some_and(|ty| {
            ty.walk().any(|arg| {
                arg.as_type()
                    .and_then(adt_of)
                    .is_some_and(|did| self.anchors.event_sources.contains(&did))
            })
        });
        here || match expr.kind {
            ExprKind::MethodCall(_, receiver, args, _) => {
                self.mentions_event(receiver) || args.iter().any(|arg| self.mentions_event(arg))
            }
            ExprKind::Call(_, args) => args.iter().any(|arg| self.mentions_event(arg)),
            ExprKind::Binary(_, left, right) => {
                self.mentions_event(left) || self.mentions_event(right)
            }
            ExprKind::Unary(_, inner)
            | ExprKind::AddrOf(_, _, inner)
            | ExprKind::DropTemps(inner)
            | ExprKind::Field(inner, _) => self.mentions_event(inner),
            ExprKind::Let(binding) => self.mentions_event(binding.init),
            _ => false,
        }
    }

    fn change_guard(&self, expr: &'tcx Expr<'tcx>, wanted: Polarity) -> bool {
        match expr.kind {
            ExprKind::Unary(UnOp::Not, inner) => self.change_guard(inner, wanted.flipped()),
            ExprKind::DropTemps(inner) => self.change_guard(inner, wanted),
            ExprKind::Binary(op, left, right) => {
                let either = matches!(
                    (op.node, wanted),
                    (BinOpKind::And, Polarity::Changed) | (BinOpKind::Or, Polarity::Unchanged)
                );
                let joined = matches!(op.node, BinOpKind::And | BinOpKind::Or);
                joined
                    && if either {
                        self.change_guard(left, wanted) || self.change_guard(right, wanted)
                    } else {
                        self.change_guard(left, wanted) && self.change_guard(right, wanted)
                    }
            }
            _ => wanted == Polarity::Changed && self.is_change_check(expr),
        }
    }

    fn is_change_check(&self, expr: &'tcx Expr<'tcx>) -> bool {
        call(self.typeck, expr).is_some_and(|Call { callee, .. }| {
            self.tcx.crate_name(callee.krate).as_str() == "bevy_ecs"
                && matches!(
                    self.tcx.item_name(callee).as_str(),
                    "is_changed" | "is_added"
                )
        })
    }

    fn owner(&self) -> LocalDefId {
        self.owners.last().copied().unwrap_or(self.root)
    }

    fn root(&self, place: &Expr<'_>) -> Option<String> {
        let mut current = place;
        loop {
            current = match current.kind {
                ExprKind::Field(inner, _)
                | ExprKind::Unary(UnOp::Deref, inner)
                | ExprKind::Index(inner, ..) => inner,
                ExprKind::Path(_) => {
                    return self
                        .tcx
                        .sess
                        .source_map()
                        .span_to_snippet(current.span)
                        .ok();
                }
                _ => return None,
            };
        }
    }

    fn was_moved(&self, place: &Expr<'_>) -> bool {
        let owner = self.owner();
        self.root(place).is_some_and(|root| {
            self.moved
                .iter()
                .any(|(moved_owner, moved)| *moved_owner == owner && *moved == root)
        })
    }

    fn counts(&self, place: &Expr<'_>, value: &Expr<'_>) -> bool {
        let source = self.tcx.sess.source_map();
        let same = |other: &Expr<'_>| {
            matches!(
                (source.span_to_snippet(place.span), source.span_to_snippet(other.span)),
                (Ok(place), Ok(other)) if place == other
            )
        };
        let nonzero = |step: &Expr<'_>| matches!(step.kind, ExprKind::Lit(lit) if matches!(lit.node, LitKind::Int(value, _) if value.get() != 0));
        match value.kind {
            ExprKind::Binary(op, left, right) => {
                matches!(op.node, BinOpKind::Add | BinOpKind::Sub) && same(left) && nonzero(right)
            }
            ExprKind::MethodCall(segment, receiver, [step], _) => {
                matches!(segment.ident.name.as_str(), "wrapping_add" | "wrapping_sub")
                    && same(receiver)
                    && nonzero(step)
            }
            _ => false,
        }
    }

    fn negates(&self, place: &'tcx Expr<'tcx>, value: &'tcx Expr<'tcx>) -> bool {
        let ExprKind::Unary(UnOp::Not, inner) = value.kind else {
            return false;
        };
        let source = self.tcx.sess.source_map();
        matches!(
            (source.span_to_snippet(place.span), source.span_to_snippet(inner.span)),
            (Ok(place), Ok(inner)) if place == inner
        )
    }

    fn check_assign(&mut self, expr: &'tcx Expr<'tcx>, place: &'tcx Expr<'tcx>) {
        if self.test
            || self.compared > 0
            || !self.through_change_tracked(place)
            || in_cfg_test(self.tcx, expr.hir_id)
        {
            return;
        }
        let ty = self.typeck.expr_ty(place);
        if !self.is_partial_eq(ty) {
            return;
        }
        emit(
            self.tcx,
            MISSED_SET_IF_NEQ,
            expr.hir_id,
            expr.span,
            |diag| {
                diag.primary_message(
                    "a write through `DerefMut` marks the value changed even when it is equal",
                );
                diag.help("use `set_if_neq`, or `map_unchanged(|v| &mut v.field).set_if_neq(..)` for one field");
            },
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
        self.anchors
            .partial_eq
            .is_some_and(|partial_eq| implements(self.tcx, self.root, partial_eq, [ty, ty]))
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Polarity {
    Changed,
    Unchanged,
}

impl Polarity {
    fn flipped(self) -> Polarity {
        match self {
            Polarity::Changed => Polarity::Unchanged,
            Polarity::Unchanged => Polarity::Changed,
        }
    }
}

fn compares(expr: &Expr<'_>) -> bool {
    match expr.kind {
        ExprKind::Binary(op, left, right) => {
            matches!(
                op.node,
                BinOpKind::Eq
                    | BinOpKind::Ne
                    | BinOpKind::Lt
                    | BinOpKind::Le
                    | BinOpKind::Gt
                    | BinOpKind::Ge
            ) || compares(left)
                || compares(right)
        }
        ExprKind::Unary(UnOp::Not, _) => true,
        ExprKind::Unary(_, inner) | ExprKind::DropTemps(inner) => compares(inner),
        ExprKind::MethodCall(segment, ..) => matches!(
            segment.ident.name.as_str(),
            "eq" | "ne" | "lt" | "le" | "gt" | "ge" | "is_none" | "is_some" | "set_if_neq"
        ),
        _ => false,
    }
}

fn early_exit<'tcx>(stmt: &Stmt<'tcx>) -> Option<&'tcx Expr<'tcx>> {
    match stmt.kind {
        StmtKind::Let(_) | StmtKind::Item(_) => None,
        StmtKind::Expr(expr) | StmtKind::Semi(expr) => {
            let ExprKind::If(condition, then, None) = expr.kind else {
                return None;
            };
            let ExprKind::Block(block, _) = then.kind else {
                return None;
            };
            let last = block.expr.or_else(|| {
                block.stmts.last().and_then(|stmt| match stmt.kind {
                    StmtKind::Expr(expr) | StmtKind::Semi(expr) => Some(expr),
                    _ => None,
                })
            })?;
            matches!(
                last.kind,
                ExprKind::Ret(_) | ExprKind::Continue(_) | ExprKind::Break(..)
            )
            .then_some(condition)
        }
    }
}

fn calls_is_finished(expr: &Expr<'_>) -> bool {
    const METHOD: &str = "is_finished";
    match expr.kind {
        ExprKind::MethodCall(segment, receiver, args, _) => {
            segment.ident.name.as_str() == METHOD
                || calls_is_finished(receiver)
                || args.iter().any(calls_is_finished)
        }
        ExprKind::Path(QPath::Resolved(_, path)) => path
            .segments
            .last()
            .is_some_and(|segment| segment.ident.name.as_str() == METHOD),
        ExprKind::Path(QPath::TypeRelative(_, segment)) => segment.ident.name.as_str() == METHOD,
        ExprKind::Let(binding) => calls_is_finished(binding.init),
        ExprKind::Unary(_, inner) | ExprKind::DropTemps(inner) => calls_is_finished(inner),
        ExprKind::Binary(_, left, right) => calls_is_finished(left) || calls_is_finished(right),
        _ => false,
    }
}

fn contains_unit(ty: Ty<'_>) -> bool {
    match ty.kind() {
        TyKind::Tuple(items) => items.is_empty() || items.iter().any(contains_unit),
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
