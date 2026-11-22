use std::path::PathBuf;

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
use crate::body_scan;
use crate::collect::collect;
use crate::collect::helper_crate;
use crate::lints;
use crate::resolve::resolve;
use crate::signature_rules;
use crate::system_rules;
use mcrs_lint::Facts;

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
        let resolved = resolve(tcx, collect(tcx, &anchors, &helper_crates), &deps);
        let bodies = body_scan::scan(tcx, &anchors);
        let tick_schedules = system_rules::tick_schedules();
        let context = system_rules::Context {
            tcx,
            anchors: &anchors,
            graph: &resolved.graph,
            bodies: &bodies,
            tick_schedules: &tick_schedules,
        };
        system_rules::check(&context, &resolved.uses, &resolved.observers);
        signature_rules::check(tcx, &anchors);
        if let Some(path) = facts_path(tcx, LOCAL_CRATE)
            && let Err(error) = resolved.facts.store(&path)
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
