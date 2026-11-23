use std::collections::HashMap;
use std::collections::HashSet;
use std::collections::VecDeque;

use rustc_hir::HirId;
use rustc_hir::def_id::DefId;
use rustc_middle::ty::Ty;
use rustc_middle::ty::TyCtxt;
use rustc_span::Span;

use crate::callbacks::DEBUG_ENV;
use crate::collect::Collected;
use crate::collect::Leaf;
use crate::collect::LeafKind;
use crate::collect::Site;
use crate::collect::key;
use mcrs_lint::Facts;
use mcrs_lint::Helper;
use mcrs_lint::Placement;
use mcrs_lint::SetFact;
use mcrs_lint::SetGraph;

pub struct SystemUse<'tcx> {
    pub def: DefId,
    pub inputs: Vec<Ty<'tcx>>,
    pub hir_id: HirId,
    pub span: Span,
    pub schedule: Option<String>,
    pub placement: Placement,
}

pub struct Resolved<'tcx> {
    pub uses: Vec<SystemUse<'tcx>>,
    pub observers: Vec<Leaf<'tcx>>,
    pub graph: SetGraph,
    pub helpers: Vec<Helper>,
    pub sets: Vec<SetFact>,
}

pub fn resolve<'tcx>(
    tcx: TyCtxt<'tcx>,
    collected: Collected<'tcx>,
    deps: &[Facts],
) -> Resolved<'tcx> {
    let local_sets: Vec<SetFact> = collected
        .sets
        .into_iter()
        .map(|(set, placement)| SetFact { set, placement })
        .collect();
    let mut graph = SetGraph::default();
    for fact in deps.iter().flat_map(|facts| &facts.sets).chain(&local_sets) {
        graph.add(fact);
    }

    let queue: VecDeque<Helper> = deps
        .iter()
        .flat_map(|facts| facts.helpers.iter().cloned())
        .collect();
    let mut state = State {
        tcx,
        uses: Vec::new(),
        seen: queue.iter().cloned().collect(),
        queue,
        local_helpers: Vec::new(),
    };
    for registration in &collected.registrations {
        for (leaf, placement) in registration.node.placements() {
            state.place(leaf, registration.schedule.as_ref(), &placement);
        }
    }
    let mut sites: HashMap<String, Vec<&Site<'tcx>>> = HashMap::new();
    for site in &collected.sites {
        sites.entry(key(tcx, site.owner)).or_default().push(site);
    }
    while let Some(helper) = state.queue.pop_front() {
        for site in sites.get(&helper.owner).into_iter().flatten() {
            for (_, leaf) in site.args.iter().filter(|(index, _)| *index == helper.index) {
                state.place(leaf.clone(), helper.schedule.as_ref(), &helper.placement);
            }
        }
    }

    Resolved {
        uses: state.uses,
        observers: collected.observers,
        graph,
        helpers: state.local_helpers,
        sets: local_sets,
    }
}

struct State<'tcx> {
    tcx: TyCtxt<'tcx>,
    uses: Vec<SystemUse<'tcx>>,
    seen: HashSet<Helper>,
    queue: VecDeque<Helper>,
    local_helpers: Vec<Helper>,
}

impl<'tcx> State<'tcx> {
    fn place(&mut self, leaf: Leaf<'tcx>, schedule: Option<&String>, placement: &Placement) {
        match leaf.kind {
            LeafKind::System { def, inputs } => self.uses.push(SystemUse {
                def,
                inputs,
                hir_id: leaf.hir_id,
                span: leaf.span,
                schedule: schedule.cloned(),
                placement: placement.clone(),
            }),
            LeafKind::Param { owner, index } => {
                let helper = Helper {
                    owner: key(self.tcx, owner),
                    index,
                    schedule: schedule.cloned(),
                    placement: placement.clone(),
                };
                if self.seen.insert(helper.clone()) {
                    self.local_helpers.push(helper.clone());
                    self.queue.push_back(helper);
                }
            }
            LeafKind::Unknown => {
                #[expect(
                    clippy::disallowed_methods,
                    reason = "cargo passes the lint configuration from `[env]` to the driver"
                )]
                let debug = std::env::var_os(DEBUG_ENV).is_some();
                if debug {
                    self.tcx.dcx().span_note(
                        leaf.span,
                        "mcrs_lint: this system configuration is not modelled",
                    );
                }
            }
        }
    }
}
