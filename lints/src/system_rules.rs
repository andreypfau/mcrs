use rustc_ast::LitKind;
use rustc_data_structures::fx::FxHashMap;
use rustc_data_structures::fx::FxHashSet;
use rustc_data_structures::fx::FxIndexMap;
use rustc_errors::Diag;
use rustc_errors::MultiSpan;
use rustc_hir::HirId;
use rustc_hir::def_id::DefId;
use rustc_hir::def_id::LocalDefId;
use rustc_lint::Level;
use rustc_lint::Lint;
use rustc_middle::ty::Ty;
use rustc_middle::ty::TyCtxt;
use rustc_span::Span;
use rustc_span::Symbol;

use crate::anchors::Anchors;
use crate::anchors::adt_of;
use crate::anchors::in_cfg_test;
use crate::anchors::type_arg_of;
use crate::body_scan::BodyFacts;
use crate::body_scan::BodyMap;
use crate::callbacks::TICK_SCHEDULES_ENV;
use crate::collect::Leaf;
use crate::collect::LeafKind;
use crate::collect::key;
use crate::facts::AlwaysChanged;
use crate::lints::ALLOC_IN_TICK;
use crate::lints::ALLOC_SITE_COUNT;
use crate::lints::BLOCKING_IN_SYSTEM;
use crate::lints::EXCLUSIVE_OBSERVER;
use crate::lints::EXCLUSIVE_SYSTEM;
use crate::lints::MARKER_TOGGLE;
use crate::lints::PER_FRAME_INSERT_RESOURCE;
use crate::lints::TIME_CLOCK_MISMATCH;
use crate::lints::UNGATED_SYSTEM;
use crate::resolve::SystemUse;
use crate::set_graph::SetGraph;

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
    for system_use in uses
        .iter()
        .filter(|system_use| !in_cfg_test(cx.tcx, system_use.hir_id))
    {
        by_system
            .entry(system_use.def)
            .or_default()
            .push(system_use);
    }
    for (def, uses) in by_system {
        always_changed.extend(check_system(cx, def, &uses));
    }
    let mut seen = FxHashSet::default();
    for observer in observers
        .iter()
        .filter(|observer| !in_cfg_test(cx.tcx, observer.hir_id))
    {
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
    uses: &[&SystemUse<'tcx>],
) -> Vec<AlwaysChanged> {
    let Some(first) = uses.first() else {
        return Vec::new();
    };
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

    let direct = def
        .as_local()
        .map(|local| reach(cx.bodies, local, Follow::Closures));
    let ungated = tick
        .iter()
        .find(|system_use| {
            !cx.graph
                .gates(&system_use.placement, system_use.schedule.as_deref())
        })
        .filter(|_| !inputs.iter().any(|ty| cx.anchors.is_gate_param(*ty)));
    let always_changed: Vec<AlwaysChanged> = if let Some(ungated) = ungated {
        let schedule = ungated.schedule.as_deref().unwrap_or_default();
        emit(tcx, UNGATED_SYSTEM, &target, target.span, |diag| {
            diag.primary_message(format!(
                "system `{}` runs every `{schedule}` without a run condition",
                target.name
            ));
            diag.help("gate it with `run_if`, a gated set, or a `Single`/`Populated` parameter");
        });
        let written = match &direct {
            Some(direct) => writes_of(cx.tcx, direct),
            None => cx
                .dep_writes
                .get(&key(cx.tcx, def))
                .cloned()
                .unwrap_or_default(),
        };
        written
            .into_iter()
            .map(|component| AlwaysChanged {
                component,
                writer: target.name.clone(),
            })
            .collect()
    } else {
        Vec::new()
    };

    check_clock(cx, &target, inputs, &schedule);
    let on_event = inputs.iter().any(|ty| cx.anchors.filters_change(*ty))
        || tick.iter().all(|system_use| {
            cx.graph
                .runs_on_event(&system_use.placement, system_use.schedule.as_deref())
        });

    let (Some(local), Some(direct)) = (def.as_local(), direct) else {
        return always_changed;
    };
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

    let mut sites: Vec<Span> = deep
        .iter()
        .flat_map(|facts| facts.allocs.iter().map(|span| span.source_callsite()))
        .collect();
    let mut seen_sites = FxHashSet::default();
    sites.retain(|site| seen_sites.insert(*site));
    if !on_event && !sites.is_empty() {
        let count = sites.len();
        let noun = if count == 1 { "site" } else { "sites" };
        emit_sites(tcx, ALLOC_IN_TICK, &target, &sites, |diag| {
            diag.primary_message(format!(
                "system `{}` allocates a heap collection on each `{schedule}` run ({count} {noun})",
                target.name
            ));
            diag.help("keep the buffer in a `Local` or a resource and `clear()` it");
        });
        let stated = stated_sites(tcx, target.hir_id);
        let expected = tcx
            .lint_level_spec_at_node(ALLOC_IN_TICK, target.hir_id)
            .level()
            == Level::Expect;
        if expected && stated != Some(count) {
            emit_sites(tcx, ALLOC_SITE_COUNT, &target, &sites, |diag| {
                let found = match stated {
                    Some(stated) => format!("the expectation states {stated}"),
                    None => "the expectation states no count".to_owned(),
                };
                diag.primary_message(format!(
                    "system `{}` allocates at {count} {noun}, {found}",
                    target.name
                ));
                diag.help(format!(
                    "state the number with `#[cfg_attr(mcrs_lint, mcrs::alloc_sites({count}))]` next to the expectation"
                ));
            });
        }
    }

    if let Some(span) = direct
        .iter()
        .flat_map(|facts| &facts.resource_inserts)
        .next()
        .filter(|_| !on_event)
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

pub fn local_writes(tcx: TyCtxt<'_>, bodies: &BodyMap, local: LocalDefId) -> Vec<String> {
    writes_of(tcx, &reach(bodies, local, Follow::Closures))
}

fn writes_of(tcx: TyCtxt<'_>, reached: &[&BodyFacts]) -> Vec<String> {
    let mut components: Vec<String> = reached
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
    adt_of(type_arg_of(type_arg_of(ty, anchors.res)?, time)?)
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
    let span = span.source_callsite();
    let mut spans = MultiSpan::from_span(span);
    if span != target.span {
        spans.push_span_label(target.span, "the system");
    }
    crate::lints::emit(tcx, lint, target.hir_id, spans, decorate);
}

fn emit_sites(
    tcx: TyCtxt<'_>,
    lint: &'static Lint,
    target: &Target,
    sites: &[Span],
    decorate: impl FnOnce(&mut Diag<'_, ()>),
) {
    let Some(first) = sites.first() else {
        return;
    };
    let mut spans = MultiSpan::from_span(first.source_callsite());
    for site in sites {
        spans.push_span_label(site.source_callsite(), "allocation site");
    }
    spans.push_span_label(target.span, "the system");
    crate::lints::emit(tcx, lint, target.hir_id, spans, decorate);
}

fn stated_sites(tcx: TyCtxt<'_>, hir_id: HirId) -> Option<usize> {
    let tool = Symbol::intern("mcrs");
    let name = Symbol::intern("alloc_sites");
    std::iter::once(hir_id)
        .chain(tcx.hir_parent_id_iter(hir_id))
        .flat_map(|id| tcx.hir_attrs(id))
        .filter(|attr| attr.path_matches(&[tool, name]))
        .find_map(|attr| {
            let items = attr.meta_item_list()?;
            let [item] = items.as_slice() else {
                return None;
            };
            match item.lit()?.kind {
                LitKind::Int(count, _) => usize::try_from(count.get()).ok(),
                _ => None,
            }
        })
}

#[expect(
    clippy::disallowed_methods,
    reason = "cargo passes the lint configuration from `[env]` to the driver"
)]
pub fn tick_schedules() -> Vec<String> {
    std::env::var(TICK_SCHEDULES_ENV)
        .map(|list| list.split(',').map(str::to_owned).collect())
        .unwrap_or_default()
}

fn is_tick(cx: &Context<'_, '_>, schedule: Option<&str>) -> bool {
    schedule.is_some_and(|name| {
        BEVY_TICK_SCHEDULES.contains(&name) || cx.tick_schedules.iter().any(|tick| tick == name)
    })
}
