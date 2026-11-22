use std::env;

use rustc_data_structures::fx::FxIndexMap as IndexMap;
use rustc_errors::Diag;
use rustc_errors::DiagDecorator;
use rustc_hir::def_id::DefId;
use rustc_lint::Lint;
use rustc_middle::ty::TyCtxt;

use crate::anchors::Anchors;
use crate::resolve::SystemUse;
use mcrs_lint::SetGraph;

pub static EXCLUSIVE_SYSTEM: &Lint = &Lint {
    name: "mcrs::exclusive_system",
    default_level: rustc_lint::Level::Warn,
    desc: "a system takes `&mut World` and runs alone",
    is_externally_loaded: true,
    eval_always: true,
    ..Lint::default_fields_for_macro()
};

pub static UNGATED_SYSTEM: &Lint = &Lint {
    name: "mcrs::ungated_system",
    default_level: rustc_lint::Level::Warn,
    desc: "a system in a tick schedule has no run condition",
    is_externally_loaded: true,
    eval_always: true,
    ..Lint::default_fields_for_macro()
};

pub static LINTS: [&Lint; 2] = [EXCLUSIVE_SYSTEM, UNGATED_SYSTEM];

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

pub fn check<'tcx>(
    tcx: TyCtxt<'tcx>,
    anchors: &Anchors,
    uses: &[SystemUse<'tcx>],
    graph: &SetGraph,
) {
    let mut by_system: IndexMap<DefId, Vec<&SystemUse<'tcx>>> = IndexMap::default();
    for system_use in uses {
        by_system
            .entry(system_use.def)
            .or_default()
            .push(system_use);
    }
    for (def, uses) in by_system {
        let Some(first) = uses.first() else {
            continue;
        };
        let at = def
            .as_local()
            .filter(|_| !tcx.is_closure_like(def))
            .map(|local| {
                (
                    tcx.local_def_id_to_hir_id(local),
                    tcx.def_ident_span(def).unwrap_or(first.span),
                )
            });
        let name = if tcx.is_closure_like(def) {
            tcx.sess
                .source_map()
                .span_to_snippet(first.span)
                .ok()
                .and_then(|snippet| snippet.lines().next().map(str::to_owned))
                .unwrap_or_else(|| tcx.def_path_str(def))
        } else {
            tcx.def_path_str(def)
        };

        if first.inputs.iter().any(|ty| anchors.is_mut_world(*ty)) {
            let (hir_id, span) = at.unwrap_or((first.hir_id, first.span));
            tcx.emit_node_span_lint(EXCLUSIVE_SYSTEM, hir_id, span, DiagDecorator(|diag: &mut Diag<'_, ()>| {
                diag.primary_message(format!("system `{name}` takes `&mut World`"));
                diag.help("use `Commands`, a query or a `SystemParam`; keep `&mut World` for a seam and state it in `#[expect]`");
            }));
        }

        if first.inputs.iter().any(|ty| anchors.is_gate_param(*ty)) {
            continue;
        }
        let ungated: Vec<&&SystemUse<'tcx>> = uses
            .iter()
            .filter(|system_use| {
                is_tick(system_use.schedule.as_deref()) && !graph.gates(&system_use.placement)
            })
            .collect();
        let Some(example) = ungated.first() else {
            continue;
        };
        let schedule = example.schedule.as_deref().unwrap_or_default();
        let targets: Vec<_> = match at {
            Some(target) => vec![target],
            None => ungated
                .iter()
                .map(|system_use| (system_use.hir_id, system_use.span))
                .collect(),
        };
        for (hir_id, span) in targets {
            tcx.emit_node_span_lint(
                UNGATED_SYSTEM,
                hir_id,
                span,
                DiagDecorator(|diag: &mut Diag<'_, ()>| {
                    diag.primary_message(format!(
                        "system `{name}` runs every `{schedule}` without a run condition"
                    ));
                    diag.help(
                        "gate it with `run_if`, a gated set, or a `Single`/`Populated` parameter",
                    );
                }),
            );
        }
    }
}

#[expect(
    clippy::disallowed_methods,
    reason = "cargo passes the lint configuration from `[env]` to the driver"
)]
fn is_tick(schedule: Option<&str>) -> bool {
    let Some(name) = schedule else {
        return false;
    };
    BEVY_TICK_SCHEDULES.contains(&name)
        || env::var("MCRS_LINT_TICK_SCHEDULES")
            .is_ok_and(|list| list.split(',').any(|configured| configured == name))
}
