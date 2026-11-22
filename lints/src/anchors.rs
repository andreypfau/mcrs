use rustc_hir::def_id::DefId;
use rustc_middle::ty::Ty;
use rustc_middle::ty::TyCtxt;
use rustc_middle::ty::TyKind;
use rustc_span::def_id::CrateNum;

pub struct Anchors {
    pub world: DefId,
    pub into_schedule_configs: DefId,
    pub registrars: Vec<DefId>,
    pub gate_params: Vec<DefId>,
}

const ECS_REGISTRARS: [&str; 2] = [
    "bevy_ecs::schedule::Schedule",
    "bevy_ecs::schedule::Schedules",
];
const APP_REGISTRARS: [&str; 2] = ["bevy_app::App", "bevy_app::SubApp"];
const GATE_PARAMS: [&str; 4] = [
    "bevy_ecs::system::Single",
    "bevy_ecs::system::Populated",
    "bevy_ecs::system::If",
    "bevy_ecs::message::PopulatedMessageReader",
];

impl Anchors {
    pub fn resolve(tcx: TyCtxt<'_>) -> Option<Anchors> {
        find_crate(tcx, "bevy_ecs")?;
        let mut missing = Vec::new();
        let mut get = |path: &str| {
            let found = resolve_path(tcx, path);
            if found.is_none() {
                missing.push(path.to_owned());
            }
            found
        };
        let world = get("bevy_ecs::world::World");
        let into_schedule_configs = get("bevy_ecs::schedule::IntoScheduleConfigs");
        let mut registrars: Vec<DefId> =
            ECS_REGISTRARS.iter().filter_map(|path| get(path)).collect();
        if find_crate(tcx, "bevy_app").is_some() {
            registrars.extend(APP_REGISTRARS.iter().filter_map(|path| get(path)));
        }
        let gate_params = GATE_PARAMS.iter().filter_map(|path| get(path)).collect();
        for path in &missing {
            tcx.dcx().err(format!(
                "mcrs_lint: the anchor `{path}` does not resolve in this Bevy version"
            ));
        }
        Some(Anchors {
            world: world?,
            into_schedule_configs: into_schedule_configs?,
            registrars,
            gate_params,
        })
    }

    pub fn is_mut_world(&self, ty: Ty<'_>) -> bool {
        matches!(ty.kind(), TyKind::Ref(_, inner, mutability)
            if mutability.is_mut() && adt_of(*inner) == Some(self.world))
    }

    pub fn is_gate_param(&self, ty: Ty<'_>) -> bool {
        adt_of(ty).is_some_and(|did| self.gate_params.contains(&did))
    }
}

pub fn adt_of(ty: Ty<'_>) -> Option<DefId> {
    match ty.peel_refs().kind() {
        TyKind::Adt(adt, _) => Some(adt.did()),
        _ => None,
    }
}

fn find_crate(tcx: TyCtxt<'_>, name: &str) -> Option<CrateNum> {
    tcx.crates(())
        .iter()
        .copied()
        .find(|krate| tcx.crate_name(*krate).as_str() == name)
}

fn resolve_path(tcx: TyCtxt<'_>, path: &str) -> Option<DefId> {
    let mut segments = path.split("::");
    let mut current = find_crate(tcx, segments.next()?)?.as_def_id();
    for segment in segments {
        current = tcx
            .module_children(current)
            .iter()
            .find(|child| child.ident.name.as_str() == segment)?
            .res
            .opt_def_id()?;
    }
    Some(current)
}
