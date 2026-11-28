use std::mem;
use std::path::PathBuf;

use rustc_data_structures::fx::FxHashMap;
use rustc_data_structures::fx::FxHashSet;
use rustc_hir::def::DefKind;
use rustc_lint::LateContext;
use rustc_lint::LateLintPass;
use rustc_lint::LintPass;
use rustc_lint::LintVec;
use rustc_middle::ty::TyCtxt;
use rustc_session::config::OutFileName;
use rustc_session::config::OutputType;
use rustc_span::def_id::CrateNum;
use rustc_span::def_id::LOCAL_CRATE;

use crate::anchors::Anchors;
use crate::anchors::system_param_trait;
use crate::arguments_rule;
use crate::body_scan;
use crate::collect::collect;
use crate::collect::helper_crate;
use crate::collect::key;
use crate::event_conditions;
use crate::facts::Facts;
use crate::facts::SystemWrites;
use crate::lints;
use crate::resolve::resolve;
use crate::signature_rules;
use crate::system_rules;

pub struct EcsLint;

impl LintPass for EcsLint {
    fn name(&self) -> &'static str {
        "EcsLint"
    }

    fn get_lints(&self) -> LintVec {
        lints::LINTS.to_vec()
    }
}

impl<'tcx> LateLintPass<'tcx> for EcsLint {
    fn check_crate_post(&mut self, cx: &LateContext<'tcx>) {
        let tcx = cx.tcx;
        arguments_rule::check(tcx, system_param_trait(tcx));
        let Some(anchors) = Anchors::resolve(tcx) else {
            return;
        };
        let deps: Vec<Facts> = tcx
            .crates(())
            .iter()
            .filter_map(|krate| facts_path(tcx, *krate))
            .filter_map(|path| Facts::read(&path).ok())
            .collect();
        let helper_crates: Vec<u64> = deps
            .iter()
            .flat_map(|facts| &facts.helpers)
            .filter_map(|helper| helper_crate(&helper.owner))
            .collect();
        let dep_writes: FxHashMap<String, Vec<String>> = deps
            .iter()
            .flat_map(|facts| &facts.writes)
            .map(|writes| (writes.system.clone(), writes.components.clone()))
            .collect();
        let marked = event_conditions::scan(tcx);
        let event_conditions: FxHashSet<String> = deps
            .iter()
            .flat_map(|facts| facts.event_conditions.iter().cloned())
            .chain(marked.valid.iter().map(|def| key(tcx, def.to_def_id())))
            .collect();
        let mut collected = collect(tcx, &anchors, &helper_crates, &event_conditions);
        marked.report(tcx, &mem::take(&mut collected.condition_uses));
        let resolved = resolve(tcx, collected, &deps);
        let test = is_test_target();
        let bodies = body_scan::scan(tcx, &anchors, test);
        let tick_schedules = system_rules::tick_schedules();
        let context = system_rules::Context {
            tcx,
            anchors: &anchors,
            graph: &resolved.graph,
            bodies: &bodies,
            tick_schedules: &tick_schedules,
            dep_writes: &dep_writes,
        };
        let local_always = if test {
            Vec::new()
        } else {
            system_rules::check(&context, &resolved.uses, &resolved.observers)
        };
        let always_changed: FxHashMap<String, String> = deps
            .iter()
            .flat_map(|facts| &facts.always_changed)
            .chain(&local_always)
            .map(|always| (always.component.clone(), always.writer.clone()))
            .collect();
        signature_rules::check(tcx, &anchors, &always_changed);

        let writes = bodies
            .keys()
            .filter(|owner| matches!(tcx.def_kind(**owner), DefKind::Fn | DefKind::AssocFn))
            .filter_map(|owner| {
                let components = system_rules::local_writes(tcx, &bodies, *owner);
                (!components.is_empty()).then(|| SystemWrites {
                    system: key(tcx, owner.to_def_id()),
                    components,
                })
            })
            .collect();
        let facts = Facts {
            helpers: resolved.helpers,
            sets: resolved.sets,
            writes,
            always_changed: local_always,
            event_conditions: marked
                .valid
                .iter()
                .map(|def| key(tcx, def.to_def_id()))
                .collect(),
        };
        if let Some(path) = facts_path(tcx, LOCAL_CRATE)
            && let Err(error) = facts.store(&path)
        {
            tcx.dcx().warn(format!(
                "mcrs_lint: cannot write {}: {error}",
                path.display()
            ));
        }
    }
}

fn facts_path(tcx: TyCtxt<'_>, krate: CrateNum) -> Option<PathBuf> {
    let metadata = if krate == LOCAL_CRATE {
        match tcx.output_filenames(()).path(OutputType::Metadata) {
            OutFileName::Real(path) => path,
            OutFileName::Stdout => return None,
        }
    } else {
        let source = tcx.used_crate_source(krate);
        source.rmeta.clone().or_else(|| source.rlib.clone())?
    };
    Some(metadata.with_extension("mcrs-lint.json"))
}

#[expect(
    clippy::disallowed_methods,
    reason = "cargo sets `CARGO_TARGET_TMPDIR` only for integration tests and benchmarks"
)]
fn is_test_target() -> bool {
    std::env::var_os("CARGO_TARGET_TMPDIR").is_some()
}
