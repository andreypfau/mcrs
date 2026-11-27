use rustc_data_structures::fx::FxHashSet;
use rustc_hir::BodyId;
use rustc_hir::Expr;
use rustc_hir::ExprKind;
use rustc_hir::HirId;
use rustc_hir::def::CtorOf;
use rustc_hir::def::DefKind;
use rustc_hir::def::Res;
use rustc_hir::def_id::DefId;
use rustc_hir::def_id::LocalDefId;
use rustc_hir::intravisit::Visitor;
use rustc_hir::intravisit::walk_expr;
use rustc_middle::hir::nested_filter;
use rustc_middle::ty::AliasTyKind;
use rustc_middle::ty::GenericArgsRef;
use rustc_middle::ty::Instance;
use rustc_middle::ty::Ty;
use rustc_middle::ty::TyCtxt;
use rustc_middle::ty::TyKind;
use rustc_middle::ty::TypeckResults;
use rustc_middle::ty::TypingEnv;
use rustc_span::Span;
use rustc_span::Symbol;
use rustc_span::sym;

use crate::anchors::Anchors;
use crate::anchors::adt_of;
use crate::config_node::ConfigNode;
use crate::config_node::Placement;

const CONDITION_CRATES: [&str; 3] = ["bevy_ecs", "bevy_input", "bevy_state"];
const EVENT_CONDITIONS: [&str; 11] = [
    "on_message",
    "resource_changed",
    "resource_exists_and_changed",
    "resource_added",
    "resource_removed",
    "resource_changed_or_removed",
    "run_once",
    "any_component_removed",
    "state_changed",
    "input_just_pressed",
    "input_just_released",
];

#[derive(Debug, Clone)]
pub enum LeafKind<'tcx> {
    System { def: DefId, inputs: Vec<Ty<'tcx>> },
    Param { owner: DefId, index: usize },
    Unknown,
}

#[derive(Debug, Clone)]
pub struct Leaf<'tcx> {
    pub kind: LeafKind<'tcx>,
    pub hir_id: HirId,
    pub span: Span,
}

pub struct Registration<'tcx> {
    pub schedule: Option<String>,
    pub node: ConfigNode<Leaf<'tcx>>,
}

pub struct Site<'tcx> {
    pub owner: DefId,
    pub args: Vec<(usize, Leaf<'tcx>)>,
}

#[derive(Default)]
pub struct Collected<'tcx> {
    pub registrations: Vec<Registration<'tcx>>,
    pub sets: Vec<(Option<String>, String, Placement)>,
    pub sites: Vec<Site<'tcx>>,
    pub observers: Vec<Leaf<'tcx>>,
    pub condition_uses: FxHashSet<DefId>,
}

pub fn collect<'tcx>(
    tcx: TyCtxt<'tcx>,
    anchors: &Anchors,
    helper_crates: &[u64],
    event_conditions: &FxHashSet<String>,
) -> Collected<'tcx> {
    let mut out = Collected::default();
    for owner in tcx.hir_body_owners() {
        if tcx.is_typeck_child(owner.to_def_id()) {
            continue;
        }
        let body = tcx.hir_body_owned_by(owner);
        let mut collector = Collector {
            tcx,
            typeck: tcx.typeck(owner),
            owner,
            anchors,
            helper_crates,
            event_conditions,
            out: &mut out,
        };
        collector.visit_body(body);
    }
    out
}

pub fn helper_crate(key: &str) -> Option<u64> {
    key.get(..16)
        .and_then(|high| u64::from_str_radix(high, 16).ok())
}

pub fn key(tcx: TyCtxt<'_>, def: DefId) -> String {
    let (high, low) = tcx.def_path_hash(def).0.split();
    format!("{:016x}{:016x}", high.as_u64(), low.as_u64())
}

struct Collector<'a, 'tcx> {
    tcx: TyCtxt<'tcx>,
    typeck: &'tcx TypeckResults<'tcx>,
    owner: LocalDefId,
    anchors: &'a Anchors,
    helper_crates: &'a [u64],
    event_conditions: &'a FxHashSet<String>,
    out: &'a mut Collected<'tcx>,
}

impl<'tcx> Visitor<'tcx> for Collector<'_, 'tcx> {
    type NestedFilter = nested_filter::OnlyBodies;

    fn maybe_tcx(&mut self) -> TyCtxt<'tcx> {
        self.tcx
    }

    fn visit_nested_body(&mut self, id: BodyId) {
        let owner = self.tcx.hir_body_owner_def_id(id);
        if self.tcx.is_typeck_child(owner.to_def_id()) {
            self.visit_body(self.tcx.hir_body(id));
        }
    }

    fn visit_expr(&mut self, expr: &'tcx Expr<'tcx>) {
        self.inspect(expr);
        walk_expr(self, expr);
    }
}

pub struct Call<'tcx> {
    pub callee: DefId,
    pub generics: GenericArgsRef<'tcx>,
    pub args: Vec<&'tcx Expr<'tcx>>,
}

pub fn call<'tcx>(typeck: &TypeckResults<'tcx>, expr: &'tcx Expr<'tcx>) -> Option<Call<'tcx>> {
    match expr.kind {
        ExprKind::MethodCall(_, receiver, args, _) => Some(Call {
            callee: typeck.type_dependent_def_id(expr.hir_id)?,
            generics: typeck.node_args(expr.hir_id),
            args: std::iter::once(receiver).chain(args.iter()).collect(),
        }),
        ExprKind::Call(func, args) => match *typeck.node_type_opt(func.hir_id)?.kind() {
            TyKind::FnDef(callee, generics) => Some(Call {
                callee,
                generics: generics.skip_binder(),
                args: args.iter().collect(),
            }),
            _ => None,
        },
        _ => None,
    }
}

impl<'tcx> Collector<'_, 'tcx> {
    fn inspect(&mut self, expr: &'tcx Expr<'tcx>) {
        if let ExprKind::Struct(..) = expr.kind
            && let TyKind::Adt(adt, generics) = self.typeck.expr_ty(expr).kind()
        {
            if self.may_forward(adt.did()) {
                self.site(adt.did(), generics, expr, &[]);
            }
            return;
        }
        let Some(call) = call(self.typeck, expr) else {
            return;
        };
        let name = self.tcx.item_name(call.callee);
        if self.is_config_method(call.callee)
            && (name.as_str().starts_with("run_if") || name.as_str() == "distributive_run_if")
            && let Some(condition) = call.args.get(1).copied()
        {
            self.note_condition_uses(condition);
        }
        if self
            .anchors
            .method_host(self.tcx, call.callee)
            .is_some_and(|host| self.anchors.registrars.contains(&host))
        {
            match (name.as_str(), call.args.as_slice()) {
                ("add_systems", [_, schedule, config]) => {
                    let node = self.config_node(config, &|this: &Self, leaf| this.leaf(leaf));
                    self.out.registrations.push(Registration {
                        schedule: self.schedule_name(schedule),
                        node,
                    });
                    return;
                }
                ("configure_sets", [_, schedule, config]) => {
                    let node = self.config_node(config, &|this: &Self, leaf| this.set_key(leaf));
                    let schedule = self.schedule_name(schedule);
                    self.out.sets.extend(
                        node.placements()
                            .into_iter()
                            .map(|(set, placement)| (schedule.clone(), set, placement)),
                    );
                    return;
                }
                _ => {}
            }
        }
        if self.anchors.is_observer_call(self.tcx, call.callee)
            && let Some(observer) = call.args.last()
        {
            let leaf = self.leaf(observer);
            self.out.observers.push(leaf);
            return;
        }
        if self.may_forward(call.callee) {
            let owner = self.site_owner(&call);
            self.site(owner, call.generics, expr, &call.args);
        }
    }

    fn may_forward(&self, def: DefId) -> bool {
        def.is_local()
            || self
                .helper_crates
                .contains(&self.tcx.stable_crate_id(def.krate).as_u64())
    }

    fn is_config_method(&self, callee: DefId) -> bool {
        self.tcx.trait_of_assoc(callee) == Some(self.anchors.into_schedule_configs)
    }

    fn site_owner(&self, call: &Call<'tcx>) -> DefId {
        if let DefKind::Ctor(CtorOf::Struct, _) = self.tcx.def_kind(call.callee) {
            return self.tcx.parent(call.callee);
        }
        if self.tcx.trait_of_assoc(call.callee).is_some() {
            let env = TypingEnv::post_analysis(self.tcx, self.owner);
            if let Ok(Some(instance)) =
                Instance::try_resolve(self.tcx, env, call.callee, call.generics)
            {
                return instance.def_id();
            }
        }
        call.callee
    }

    fn site(
        &mut self,
        owner: DefId,
        generics: GenericArgsRef<'tcx>,
        expr: &'tcx Expr<'tcx>,
        args: &[&'tcx Expr<'tcx>],
    ) {
        let leaves: Vec<(usize, Leaf<'tcx>)> = generics
            .iter()
            .enumerate()
            .filter_map(|(index, arg)| {
                let ty = arg.as_type()?;
                if !matches!(
                    ty.kind(),
                    TyKind::FnDef(..) | TyKind::Closure(..) | TyKind::Param(_)
                ) {
                    return None;
                }
                let at = args
                    .iter()
                    .find(|arg| self.typeck.expr_ty(arg).peel_refs() == ty)
                    .copied()
                    .unwrap_or(expr);
                Some((index, self.leaf_of(ty, at.hir_id, at.span)))
            })
            .collect();
        if !leaves.is_empty() {
            self.out.sites.push(Site {
                owner,
                args: leaves,
            });
        }
    }

    fn config_node<L: Clone, F: Fn(&Self, &'tcx Expr<'tcx>) -> L>(
        &self,
        expr: &'tcx Expr<'tcx>,
        leaf: &F,
    ) -> ConfigNode<L> {
        match expr.kind {
            ExprKind::Tup(children) => ConfigNode::Tuple(
                children
                    .iter()
                    .map(|child| self.config_node(child, leaf))
                    .collect(),
            ),
            ExprKind::MethodCall(segment, receiver, args, _)
                if self
                    .typeck
                    .type_dependent_def_id(expr.hir_id)
                    .is_some_and(|callee| self.is_config_method(callee)) =>
            {
                let name = segment.ident.name.as_str();
                let sets = match (name, args) {
                    ("in_set", [set]) => vec![self.set_key(set)],
                    _ => Vec::new(),
                };
                let run_if = name.starts_with("run_if") || name == "distributive_run_if";
                ConfigNode::Configured {
                    inner: Box::new(self.config_node(receiver, leaf)),
                    placement: Placement {
                        run_if,
                        on_event: run_if && args.first().is_some_and(|cond| self.on_event(cond)),
                        sets,
                    },
                }
            }
            _ => ConfigNode::Leaf(leaf(self, expr)),
        }
    }

    fn on_event(&self, condition: &'tcx Expr<'tcx>) -> bool {
        match condition.kind {
            ExprKind::MethodCall(segment, receiver, [other], _) => {
                match segment.ident.name.as_str() {
                    "and_then" | "and_eager" => self.on_event(receiver) || self.on_event(other),
                    "or_else" | "or_eager" => self.on_event(receiver) && self.on_event(other),
                    _ => false,
                }
            }
            ExprKind::Call(callee, [inner]) if self.condition_name(callee) == Some(sym::not) => {
                matches!(inner.kind, ExprKind::Path(..))
                    && self.condition_name(inner) == Some(Symbol::intern("resource_exists"))
            }
            ExprKind::Call(callee, _) => self.is_event_condition(callee),
            ExprKind::Path(..) => self.is_event_condition(condition),
            _ => false,
        }
    }

    fn note_condition_uses(&mut self, condition: &'tcx Expr<'tcx>) {
        match condition.kind {
            ExprKind::MethodCall(_, receiver, args, _) => {
                self.note_condition_uses(receiver);
                for arg in args {
                    self.note_condition_uses(arg);
                }
            }
            ExprKind::Call(callee, args) => {
                self.note_condition_uses(callee);
                for arg in args {
                    self.note_condition_uses(arg);
                }
            }
            ExprKind::Path(..) => {
                if let TyKind::FnDef(def, _) = *self.typeck.expr_ty(condition).kind() {
                    self.out.condition_uses.insert(def);
                }
            }
            _ => {}
        }
    }

    fn is_event_condition(&self, expr: &'tcx Expr<'tcx>) -> bool {
        let TyKind::FnDef(def, generics) = *self.typeck.expr_ty(expr).kind() else {
            return false;
        };
        if !CONDITION_CRATES.contains(&self.tcx.crate_name(def.krate).as_str()) {
            return self.event_conditions.contains(&key(self.tcx, def));
        }
        let name = self.tcx.item_name(def);
        EVENT_CONDITIONS.contains(&name.as_str())
            || (name.as_str() == "any_match_filter"
                && generics
                    .skip_binder()
                    .types()
                    .any(|ty| self.anchors.mentions_change(ty)))
    }

    fn condition_name(&self, expr: &'tcx Expr<'tcx>) -> Option<Symbol> {
        let TyKind::FnDef(def, _) = *self.typeck.expr_ty(expr).kind() else {
            return None;
        };
        CONDITION_CRATES
            .contains(&self.tcx.crate_name(def.krate).as_str())
            .then(|| self.tcx.item_name(def))
    }

    fn leaf(&self, expr: &'tcx Expr<'tcx>) -> Leaf<'tcx> {
        self.leaf_of(self.typeck.expr_ty(expr), expr.hir_id, expr.span)
    }

    fn leaf_of(&self, ty: Ty<'tcx>, hir_id: HirId, span: Span) -> Leaf<'tcx> {
        let tcx = self.tcx;
        let kind = match *ty.kind() {
            TyKind::FnDef(def, generics) => LeafKind::System {
                def,
                inputs: tcx
                    .fn_sig(def)
                    .instantiate(tcx, generics.skip_binder())
                    .skip_normalization()
                    .skip_binder()
                    .inputs()
                    .to_vec(),
            },
            TyKind::Closure(def, generics) => LeafKind::System {
                def,
                inputs: closure_inputs(generics.as_closure().sig().skip_binder().inputs()),
            },
            TyKind::Param(param) => self.param_leaf(ty, param.index),
            TyKind::Alias(_, alias) => match alias.kind {
                AliasTyKind::Opaque { def_id } => {
                    let hidden = tcx
                        .type_of(def_id)
                        .instantiate(tcx, alias.args)
                        .skip_normalization();
                    return self.leaf_of(hidden, hir_id, span);
                }
                _ => LeafKind::Unknown,
            },
            _ => LeafKind::Unknown,
        };
        Leaf { kind, hir_id, span }
    }

    fn param_leaf(&self, ty: Ty<'tcx>, index: u32) -> LeafKind<'tcx> {
        let tcx = self.tcx;
        let generics = tcx.generics_of(self.owner);
        let param = generics.param_at(index as usize, tcx);
        let owner = tcx.parent(param.def_id);
        if !matches!(tcx.def_kind(owner), DefKind::Impl { .. }) {
            return LeafKind::Param {
                owner,
                index: index as usize,
            };
        }
        match tcx
            .type_of(owner)
            .instantiate_identity()
            .skip_normalization()
            .kind()
        {
            TyKind::Adt(adt, args) => args
                .iter()
                .position(|arg| arg.as_type() == Some(ty))
                .map_or(LeafKind::Unknown, |index| LeafKind::Param {
                    owner: adt.did(),
                    index,
                }),
            _ => LeafKind::Unknown,
        }
    }

    fn set_key(&self, expr: &'tcx Expr<'tcx>) -> String {
        if let ExprKind::Path(ref qpath) = expr.kind
            && let Res::Def(DefKind::Ctor(_, _), ctor) = self.typeck.qpath_res(qpath, expr.hir_id)
        {
            return key(self.tcx, self.tcx.parent(ctor));
        }
        match adt_of(self.typeck.expr_ty(expr)) {
            Some(did) => key(self.tcx, did),
            None => String::new(),
        }
    }

    fn schedule_name(&self, expr: &'tcx Expr<'tcx>) -> Option<String> {
        adt_of(self.typeck.expr_ty(expr)).map(|did| self.tcx.item_name(did).to_string())
    }
}

fn closure_inputs<'tcx>(inputs: &[Ty<'tcx>]) -> Vec<Ty<'tcx>> {
    match inputs {
        [tuple] => match tuple.kind() {
            TyKind::Tuple(items) => items.to_vec(),
            _ => vec![*tuple],
        },
        _ => inputs.to_vec(),
    }
}
