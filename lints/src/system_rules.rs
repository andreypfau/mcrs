use rustc_data_structures::fx::FxHashMap;
use rustc_data_structures::fx::FxHashSet;
use rustc_data_structures::fx::FxIndexMap;
use rustc_errors::Diag;
use rustc_errors::DiagDecorator;
use rustc_errors::MultiSpan;
use rustc_hir::HirId;
use rustc_hir::def_id::DefId;
use rustc_hir::def_id::LocalDefId;
use rustc_lint::Lint;
use rustc_middle::ty::Ty;
use rustc_middle::ty::TyCtxt;
use rustc_middle::ty::TyKind;
use rustc_span::Span;

use crate::anchors::Anchors;
use crate::anchors::adt_of;
use crate::body_scan::BodyFacts;
use crate::body_scan::BodyMap;
use crate::collect::Leaf;
use crate::collect::LeafKind;
use crate::collect::key;
use crate::lints::ALLOC_IN_TICK;
use crate::lints::BLOCKING_IN_SYSTEM;
use crate::lints::EXCLUSIVE_OBSERVER;
use crate::lints::EXCLUSIVE_SYSTEM;
use crate::lints::MARKER_TOGGLE;
use crate::lints::PER_FRAME_INSERT_RESOURCE;
use crate::lints::TIME_CLOCK_MISMATCH;
use crate::lints::UNGATED_SYSTEM;
use crate::resolve::SystemUse;
use mcrs_lint::AlwaysChanged;
use mcrs_lint::SetGraph;

const BEVY_TICK_SCHEDULES: [&str; 14] = [
    "First",
    "PreUpdate",
    "RunFixedMainLoop",
    "FixedFirst",
    "FixedPreUpdate",
    "FixedUpdate",
    "FixedPostUpdate",
    "FixedLast",
    "Update",
    "SpawnScene",
    "PostUpdate",
    "Last",
    "ExtractSchedule",
    "Render",
];

pub struct Context<'a, 'tcx> {
    pub tcx: TyCtxt<'tcx>,
    pub anchors: &'a Anchors,
    pub graph: &'a SetGraph,
    pub bodies: &'a BodyMap,
    pub tick_schedules: &'a [String],
    pub dep_writes: &'a FxHashMap<String, Vec<String>>,
}

struct Target {
    hir_id: HirId,
    span: Span,
    name: String,
}

pub fn check<'tcx>(
    cx: &Context<'_, 'tcx>,
    uses: &[SystemUse<'tcx>],
    observers: &[Leaf<'tcx>],
) -> Vec<AlwaysChanged> {
    let mut always_changed = Vec::new();
    let mut by_system: FxIndexMap<DefId, Vec<&SystemUse<'tcx>>> = FxIndexMap::default();
    for system_use in uses {
        by_system
            .entry(system_use.def)
            .or_default()
            .push(system_use);
    }
    for (def, uses) in by_system {
        if let Some(first) = uses.first() {
            always_changed.extend(check_system(cx, def, first, &uses));
        }
    }
    let mut seen = FxHashSet::default();
    for observer in observers {
        let LeafKind::System { def, inputs } = &observer.kind else {
            continue;
        };
        if !seen.insert(*def) || !inputs.iter().any(|ty| cx.anchors.is_mut_world(*ty)) {
            continue;
        }
        let target = target(cx.tcx, *def, observer.hir_id, observer.span);
        emit(cx.tcx, EXCLUSIVE_OBSERVER, &target, target.span, |diag| {
            diag.primary_message(format!("observer `{}` takes `&mut World`", target.name));
            diag.help(
                "Bevy panics when it adds an exclusive observer; use `DeferredWorld` or `Commands`",
            );
        });
    }
    always_changed
}

fn check_system<'tcx>(
    cx: &Context<'_, 'tcx>,
    def: DefId,
    first: &SystemUse<'tcx>,
    uses: &[&SystemUse<'tcx>],
) -> Vec<AlwaysChanged> {
    let tcx = cx.tcx;
    let target = target(tcx, def, first.hir_id, first.span);
    let inputs = &first.inputs;

    if inputs.iter().any(|ty| cx.anchors.is_mut_world(*ty)) {
        emit(tcx, EXCLUSIVE_SYSTEM, &target, target.span, |diag| {
            diag.primary_message(format!("system `{}` takes `&mut World`", target.name));
            diag.help("use `Commands`, a query or a `SystemParam`; keep `&mut World` for a seam and state it in `#[expect]`");
        });
    }

    let tick: Vec<&&SystemUse<'tcx>> = uses
        .iter()
        .filter(|system_use| is_tick(cx, system_use.schedule.as_deref()))
        .collect();
    let Some(example) = tick.first() else {
        return Vec::new();
    };
    let schedule = example.schedule.as_deref().unwrap_or_default().to_owned();

    let ungated = tick
        .iter()
        .find(|system_use| !cx.graph.gates(&system_use.placement))
        .filter(|_| !inputs.iter().any(|ty| cx.anchors.is_gate_param(*ty)));
    let always_changed: Vec<AlwaysChanged> = if ungated.is_some() {
        written(cx, def)
            .into_iter()
            .map(|component| AlwaysChanged {
                component,
                writer: target.name.clone(),
            })
            .collect()
    } else {
        Vec::new()
    };
    if let Some(ungated) = ungated {
        let schedule = ungated.schedule.as_deref().unwrap_or_default();
        emit(tcx, UNGATED_SYSTEM, &target, target.span, |diag| {
            diag.primary_message(format!(
                "system `{}` runs every `{schedule}` without a run condition",
                target.name
            ));
            diag.help("gate it with `run_if`, a gated set, or a `Single`/`Populated` parameter");
        });
    }

    check_clock(cx, &target, inputs, &schedule);

    let Some(local) = def.as_local() else {
        return always_changed;
    };
    let direct = reach(cx.bodies, local, Follow::Closures);
    let deep = reach(cx.bodies, local, Follow::Calls);

    if let Some((span, callee)) = deep.iter().flat_map(|facts| &facts.blocking).next() {
        let callee = tcx.def_path_str(*callee);
        emit(tcx, BLOCKING_IN_SYSTEM, &target, *span, |diag| {
            diag.primary_message(format!(
                "system `{}` calls `{callee}` in `{schedule}`",
                target.name
            ));
            diag.help("read `Res<Time>`; move disk and blocking work to `IoTaskPool` and collect it with `check_ready`");
        });
    }

    let allocs: Vec<Span> = deep
        .iter()
        .flat_map(|facts| facts.allocs.iter().copied())
        .collect();
    if let Some(span) = allocs.first() {
        let count = allocs.len();
        emit(tcx, ALLOC_IN_TICK, &target, *span, |diag| {
            diag.primary_message(format!(
                "system `{}` allocates a heap collection on each `{schedule}` run ({count} {})",
                target.name,
                if count == 1 { "site" } else { "sites" }
            ));
            diag.help("keep the buffer in a `Local` or a resource and `clear()` it");
        });
    }

    if let Some(span) = direct
        .iter()
        .flat_map(|facts| &facts.resource_inserts)
        .next()
    {
        emit(tcx, PER_FRAME_INSERT_RESOURCE, &target, *span, |diag| {
            diag.primary_message(format!(
                "system `{}` inserts a resource on each `{schedule}` run",
                target.name
            ));
            diag.help("mutate it through `ResMut` and `set_if_neq`, or insert it once");
        });
    }

    let inserted: Vec<(DefId, Span)> = direct
        .iter()
        .flat_map(|facts| facts.marker_inserts.iter().copied())
        .collect();
    let removed: Vec<(DefId, Span)> = direct
        .iter()
        .flat_map(|facts| facts.marker_removes.iter().copied())
        .collect();
    if let Some((marker, span)) = inserted
        .iter()
        .find(|(marker, _)| removed.iter().any(|(other, _)| other == marker))
    {
        let marker = tcx.def_path_str(*marker);
        emit(tcx, MARKER_TOGGLE, &target, *span, |diag| {
            diag.primary_message(format!(
                "system `{}` inserts and removes `{marker}` in `{schedule}`",
                target.name
            ));
            diag.help("each toggle moves the entity to another archetype; store state that flips often as a value field");
        });
    }
    always_changed
}

fn written(cx: &Context<'_, '_>, def: DefId) -> Vec<String> {
    match def.as_local() {
        Some(local) => local_writes(cx.tcx, cx.bodies, local),
        None => cx
            .dep_writes
            .get(&key(cx.tcx, def))
            .cloned()
            .unwrap_or_default(),
    }
}

pub fn local_writes(tcx: TyCtxt<'_>, bodies: &BodyMap, local: LocalDefId) -> Vec<String> {
    let mut components: Vec<String> = reach(bodies, local, Follow::Closures)
        .iter()
        .flat_map(|facts| facts.writes.iter().map(|component| key(tcx, *component)))
        .collect();
    components.sort_unstable();
    components.dedup();
    components
}

fn check_clock(cx: &Context<'_, '_>, target: &Target, inputs: &[Ty<'_>], schedule: &str) {
    let Some(time) = &cx.anchors.time else {
        return;
    };
    if !schedule.starts_with("Fixed") {
        return;
    }
    for input in inputs {
        let Some(clock) = clock_of(cx.anchors, time.time, *input) else {
            continue;
        };
        if clock == time.real || clock == time.virtual_clock {
            let clock = cx.tcx.item_name(clock);
            emit(cx.tcx, TIME_CLOCK_MISMATCH, target, target.span, |diag| {
                diag.primary_message(format!(
                    "system `{}` reads `Time<{clock}>` in `{schedule}`",
                    target.name
                ));
                diag.help("read `Res<Time>`: in a fixed schedule it is the fixed clock");
            });
        }
    }
}

fn clock_of(anchors: &Anchors, time: DefId, ty: Ty<'_>) -> Option<DefId> {
    let TyKind::Adt(res, args) = ty.kind() else {
        return None;
    };
    if res.did() != anchors.res {
        return None;
    }
    let TyKind::Adt(time_adt, time_args) = args.types().next()?.kind() else {
        return None;
    };
    if time_adt.did() != time {
        return None;
    }
    adt_of(time_args.types().next()?)
}

#[derive(Clone, Copy, PartialEq)]
enum Follow {
    Closures,
    Calls,
}

fn reach(bodies: &BodyMap, root: LocalDefId, follow: Follow) -> Vec<&BodyFacts> {
    let mut seen = FxHashSet::default();
    let mut stack = vec![root];
    let mut out = Vec::new();
    while let Some(owner) = stack.pop() {
        if !seen.insert(owner) {
            continue;
        }
        let Some(facts) = bodies.get(&owner) else {
            continue;
        };
        out.push(facts);
        stack.extend(&facts.closures);
        if follow == Follow::Calls {
            stack.extend(&facts.calls);
        }
    }
    out
}

fn target(tcx: TyCtxt<'_>, def: DefId, site: HirId, site_span: Span) -> Target {
    let closure = tcx.is_closure_like(def);
    let local = def.as_local().filter(|_| !closure);
    let name = if closure {
        tcx.sess
            .source_map()
            .span_to_snippet(site_span)
            .ok()
            .and_then(|snippet| snippet.lines().next().map(str::to_owned))
            .unwrap_or_else(|| tcx.def_path_str(def))
    } else {
        tcx.def_path_str(def)
    };
    match local {
        Some(local) => Target {
            hir_id: tcx.local_def_id_to_hir_id(local),
            span: tcx.def_ident_span(def).unwrap_or(site_span),
            name,
        },
        None => Target {
            hir_id: site,
            span: site_span,
            name,
        },
    }
}

fn emit(
    tcx: TyCtxt<'_>,
    lint: &'static Lint,
    target: &Target,
    span: Span,
    decorate: impl FnOnce(&mut Diag<'_, ()>),
) {
    let mut spans = MultiSpan::from_span(span);
    if span != target.span {
        spans.push_span_label(target.span, "the system");
    }
    tcx.emit_node_span_lint(lint, target.hir_id, spans, DiagDecorator(decorate));
}

#[expect(
    clippy::disallowed_methods,
    reason = "cargo passes the lint configuration from `[env]` to the driver"
)]
pub fn tick_schedules() -> Vec<String> {
    std::env::var("MCRS_LINT_TICK_SCHEDULES")
        .map(|list| list.split(',').map(str::to_owned).collect())
        .unwrap_or_default()
}

fn is_tick(cx: &Context<'_, '_>, schedule: Option<&str>) -> bool {
    schedule.is_some_and(|name| {
        BEVY_TICK_SCHEDULES.contains(&name) || cx.tick_schedules.iter().any(|tick| tick == name)
    })
}
