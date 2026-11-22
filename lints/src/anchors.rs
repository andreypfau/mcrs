use rustc_hir::def::DefKind;
use rustc_hir::def_id::DefId;
use rustc_middle::ty::Ty;
use rustc_middle::ty::TyCtxt;
use rustc_middle::ty::TyKind;
use rustc_span::Symbol;
use rustc_span::def_id::CrateNum;

pub struct Anchors {
    pub world: DefId,
    pub into_schedule_configs: DefId,
    pub registrars: Vec<DefId>,
    pub observer_hosts: Vec<DefId>,
    pub gate_params: Vec<DefId>,
    pub query: DefId,
    pub res: DefId,
    pub messages: DefId,
    pub resource_hosts: Vec<DefId>,
    pub entity_hosts: Vec<DefId>,
    pub change_tracked: Vec<DefId>,
    pub reborrowable: Vec<DefId>,
    pub time: Option<TimeAnchors>,
    pub blocking_fns: Vec<DefId>,
    pub blocking_modules: Vec<DefId>,
    pub blocking_types: Vec<DefId>,
    pub heap_types: Vec<DefId>,
    pub moves: Vec<DefId>,
    pub partial_eq: Option<DefId>,
    pub bundle: DefId,
    pub changed: DefId,
    pub mut_ref: DefId,
}

pub struct TimeAnchors {
    pub time: DefId,
    pub real: DefId,
    pub virtual_clock: DefId,
}

const ECS_REGISTRARS: [&str; 2] = [
    "bevy_ecs::schedule::Schedule",
    "bevy_ecs::schedule::Schedules",
];
const APP_HOSTS: [&str; 2] = ["bevy_app::App", "bevy_app::SubApp"];
const GATE_PARAMS: [&str; 4] = [
    "bevy_ecs::system::Single",
    "bevy_ecs::system::Populated",
    "bevy_ecs::system::If",
    "bevy_ecs::message::PopulatedMessageReader",
];
const ENTITY_HOSTS: [&str; 2] = [
    "bevy_ecs::system::EntityCommands",
    "bevy_ecs::world::EntityWorldMut",
];
const CHANGE_TRACKED: [&str; 3] = [
    "bevy_ecs::change_detection::Mut",
    "bevy_ecs::change_detection::ResMut",
    "bevy_ecs::change_detection::NonSendMut",
];
const REBORROWABLE: [&str; 8] = [
    "bevy_ecs::system::Commands",
    "bevy_ecs::system::EntityCommands",
    "bevy_ecs::system::Query",
    "bevy_ecs::world::DeferredWorld",
    "bevy_ecs::world::EntityMut",
    "bevy_ecs::change_detection::Mut",
    "bevy_ecs::change_detection::ResMut",
    "bevy_ecs::change_detection::NonSendMut",
];
const BLOCKING_FNS: [&str; 5] = [
    "std::thread::sleep",
    "bevy_tasks::block_on",
    "futures_lite::future::block_on",
    "pollster::block_on",
    "futures::executor::block_on",
];
const BLOCKING_METHODS: [(&str, &str); 5] = [
    ("std::time::Instant", "now"),
    ("std::time::SystemTime", "now"),
    ("std::sync::mpsc::Receiver", "recv"),
    ("std::sync::mpsc::Receiver", "recv_timeout"),
    ("std::thread::JoinHandle", "join"),
];
const BLOCKING_TYPES: [&str; 2] = ["std::fs::File", "std::net::TcpStream"];
const HEAP_TYPES: [&str; 13] = [
    "alloc::vec::Vec",
    "alloc::string::String",
    "alloc::boxed::Box",
    "alloc::collections::VecDeque",
    "alloc::collections::BTreeMap",
    "alloc::collections::BTreeSet",
    "alloc::collections::BinaryHeap",
    "alloc::collections::LinkedList",
    "std::collections::HashMap",
    "std::collections::HashSet",
    "hashbrown::HashMap",
    "hashbrown::HashSet",
    "indexmap::IndexMap",
];
const MOVES: [&str; 3] = ["core::mem::take", "core::mem::replace", "core::mem::swap"];

impl Anchors {
    pub fn resolve(tcx: TyCtxt<'_>) -> Option<Anchors> {
        find_crate(tcx, "bevy_ecs")?;
        let has_app = find_crate(tcx, "bevy_app").is_some();
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
        let query = get("bevy_ecs::system::Query");
        let res = get("bevy_ecs::change_detection::Res");
        let messages = get("bevy_ecs::message::Messages");
        let commands = get("bevy_ecs::system::Commands");
        let bundle = get("bevy_ecs::bundle::Bundle");
        let changed = get("bevy_ecs::query::Changed");
        let mut_ref = get("bevy_ecs::change_detection::Mut");
        let mut registrars: Vec<DefId> =
            ECS_REGISTRARS.iter().filter_map(|path| get(path)).collect();
        let apps: Vec<DefId> = if has_app {
            APP_HOSTS.iter().filter_map(|path| get(path)).collect()
        } else {
            Vec::new()
        };
        registrars.extend(&apps);
        let gate_params = GATE_PARAMS.iter().filter_map(|path| get(path)).collect();
        let entity_hosts: Vec<DefId> = ENTITY_HOSTS.iter().filter_map(|path| get(path)).collect();
        let change_tracked = CHANGE_TRACKED.iter().filter_map(|path| get(path)).collect();
        let reborrowable = REBORROWABLE.iter().filter_map(|path| get(path)).collect();
        let time = if find_crate(tcx, "bevy_time").is_some() {
            match (
                get("bevy_time::Time"),
                get("bevy_time::Real"),
                get("bevy_time::Virtual"),
            ) {
                (Some(time), Some(real), Some(virtual_clock)) => Some(TimeAnchors {
                    time,
                    real,
                    virtual_clock,
                }),
                _ => None,
            }
        } else {
            None
        };
        for path in &missing {
            tcx.dcx().err(format!(
                "mcrs_lint: the anchor `{path}` does not resolve in this Bevy version"
            ));
        }
        let world = world?;
        let commands = commands?;
        let mut observer_hosts = apps.clone();
        observer_hosts.extend([world, commands]);
        observer_hosts.extend(&entity_hosts);
        let mut resource_hosts = apps;
        resource_hosts.extend([world, commands]);

        let mut blocking_fns: Vec<DefId> = BLOCKING_FNS
            .iter()
            .filter_map(|path| resolve_path(tcx, path))
            .collect();
        blocking_fns.extend(
            BLOCKING_METHODS
                .iter()
                .filter_map(|(ty, name)| resolve_method(tcx, resolve_path(tcx, ty)?, name)),
        );
        Some(Anchors {
            world,
            into_schedule_configs: into_schedule_configs?,
            registrars,
            observer_hosts,
            gate_params,
            query: query?,
            res: res?,
            messages: messages?,
            resource_hosts,
            entity_hosts,
            change_tracked,
            reborrowable,
            time,
            blocking_fns,
            blocking_modules: resolve_path(tcx, "std::fs").into_iter().collect(),
            blocking_types: BLOCKING_TYPES
                .iter()
                .filter_map(|path| resolve_path(tcx, path))
                .collect(),
            heap_types: HEAP_TYPES
                .iter()
                .filter_map(|path| resolve_path(tcx, path))
                .collect(),
            moves: MOVES
                .iter()
                .filter_map(|path| resolve_path(tcx, path))
                .collect(),
            partial_eq: tcx.get_diagnostic_item(rustc_span::sym::PartialEq),
            bundle: bundle?,
            changed: changed?,
            mut_ref: mut_ref?,
        })
    }

    pub fn is_mut_world(&self, ty: Ty<'_>) -> bool {
        matches!(ty.kind(), TyKind::Ref(_, inner, mutability)
            if mutability.is_mut() && adt_of(*inner) == Some(self.world))
    }

    pub fn is_gate_param(&self, ty: Ty<'_>) -> bool {
        adt_of(ty).is_some_and(|did| self.gate_params.contains(&did))
    }

    pub fn is_heap(&self, ty: Ty<'_>) -> bool {
        match ty.kind() {
            TyKind::Adt(adt, _) => self.heap_types.contains(&adt.did()),
            _ => false,
        }
    }

    pub fn is_blocking(&self, tcx: TyCtxt<'_>, callee: DefId) -> bool {
        if self.blocking_fns.contains(&callee) {
            return true;
        }
        if tcx
            .opt_parent(callee)
            .is_some_and(|parent| self.blocking_modules.contains(&parent))
        {
            return true;
        }
        tcx.inherent_impl_of_assoc(callee)
            .and_then(|imp| adt_of(tcx.type_of(imp).instantiate_identity().skip_normalization()))
            .is_some_and(|did| self.blocking_types.contains(&did))
    }

    pub fn method_host(&self, tcx: TyCtxt<'_>, callee: DefId) -> Option<DefId> {
        tcx.inherent_impl_of_assoc(callee)
            .and_then(|imp| adt_of(tcx.type_of(imp).instantiate_identity().skip_normalization()))
    }
}

pub fn system_param_trait(tcx: TyCtxt<'_>) -> Option<DefId> {
    resolve_path(tcx, "bevy_ecs::system::SystemParam")
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
            .filter(|child| child.ident.name.as_str() == segment)
            .filter_map(|child| child.res.opt_def_id())
            .find(|def| !matches!(tcx.def_kind(*def), DefKind::Macro(_)))?;
    }
    Some(current)
}

fn resolve_method(tcx: TyCtxt<'_>, ty: DefId, name: &str) -> Option<DefId> {
    let name = Symbol::intern(name);
    tcx.inherent_impls(ty).iter().find_map(|imp| {
        tcx.associated_items(*imp)
            .filter_by_name_unhygienic(name)
            .next()
            .map(|item| item.def_id)
    })
}
