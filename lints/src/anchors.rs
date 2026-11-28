use rustc_ast::attr::data_structures::CfgEntry;
use rustc_hir::Attribute;
use rustc_hir::HirId;
use rustc_hir::attrs::AttributeKind;
use rustc_hir::def::DefKind;
use rustc_hir::def_id::DefId;
use rustc_hir::def_id::LocalDefId;
use rustc_infer::infer::TyCtxtInferExt;
use rustc_middle::ty::Ty;
use rustc_middle::ty::TyCtxt;
use rustc_middle::ty::TyKind;
use rustc_middle::ty::TypingEnv;
use rustc_span::Symbol;
use rustc_span::def_id::CrateNum;
use rustc_trait_selection::infer::InferCtxtExt;

pub struct Anchors {
    pub world: DefId,
    pub into_schedule_configs: DefId,
    pub registrars: Vec<DefId>,
    pub observer_hosts: Vec<DefId>,
    pub gate_params: Vec<DefId>,
    pub res: DefId,
    pub messages: DefId,
    pub resource_hosts: Vec<DefId>,
    pub entity_hosts: Vec<DefId>,
    pub change_tracked: Vec<DefId>,
    pub reborrowable: Vec<DefId>,
    pub time: Option<TimeAnchors>,
    pub blocking_fns: Vec<DefId>,
    pub blocking_module: Option<DefId>,
    pub blocking_types: Vec<DefId>,
    pub heap_types: Vec<DefId>,
    pub moves: Vec<DefId>,
    pub partial_eq: Option<DefId>,
    pub bundle: DefId,
    pub changed: DefId,
    pub added: DefId,
    pub event_sources: Vec<DefId>,
    pub queries: Vec<DefId>,
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
const QUERIES: [&str; 3] = [
    "bevy_ecs::system::Query",
    "bevy_ecs::system::Single",
    "bevy_ecs::system::Populated",
];
const EVENT_SOURCES: [&str; 8] = [
    "bevy_ecs::message::Messages",
    "bevy_ecs::message::MessageReader",
    "bevy_ecs::message::MessageMutator",
    "bevy_ecs::message::MessageIterator",
    "bevy_ecs::message::MessageMutIterator",
    "bevy_ecs::lifecycle::RemovedComponents",
    "bevy_ecs::query::Changed",
    "bevy_ecs::query::Added",
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
        let res = get("bevy_ecs::change_detection::Res");
        let messages = get("bevy_ecs::message::Messages");
        let commands = get("bevy_ecs::system::Commands");
        let bundle = get("bevy_ecs::bundle::Bundle");
        let changed = get("bevy_ecs::query::Changed");
        let added = get("bevy_ecs::query::Added");
        let mut_ref = get("bevy_ecs::change_detection::Mut");
        let mut get_all =
            |paths: &[&str]| -> Vec<DefId> { paths.iter().filter_map(|path| get(path)).collect() };
        let mut registrars = get_all(&ECS_REGISTRARS);
        let apps = if has_app {
            get_all(&APP_HOSTS)
        } else {
            Vec::new()
        };
        registrars.extend(&apps);
        let gate_params = get_all(&GATE_PARAMS);
        let entity_hosts = get_all(&ENTITY_HOSTS);
        let change_tracked = get_all(&CHANGE_TRACKED);
        let reborrowable = get_all(&REBORROWABLE);
        let event_sources = get_all(&EVENT_SOURCES);
        let queries = get_all(&QUERIES);
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

        let mut blocking_fns = resolve_all(tcx, &BLOCKING_FNS);
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
            res: res?,
            messages: messages?,
            resource_hosts,
            entity_hosts,
            change_tracked,
            reborrowable,
            time,
            blocking_fns,
            blocking_module: resolve_path(tcx, "std::fs"),
            blocking_types: resolve_all(tcx, &BLOCKING_TYPES),
            heap_types: resolve_all(tcx, &HEAP_TYPES),
            moves: resolve_all(tcx, &MOVES),
            partial_eq: tcx.get_diagnostic_item(rustc_span::sym::PartialEq),
            bundle: bundle?,
            changed: changed?,
            added: added?,
            event_sources,
            queries,
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

    pub fn filters_change(&self, ty: Ty<'_>) -> bool {
        self.is_gate_param(ty) && self.mentions_change(ty)
    }

    pub fn mentions_change(&self, ty: Ty<'_>) -> bool {
        ty.walk().any(|arg| {
            arg.as_type()
                .and_then(adt_of)
                .is_some_and(|did| did == self.changed || did == self.added)
        })
    }

    pub fn query_data<'tcx>(&self, ty: Ty<'tcx>) -> Option<Ty<'tcx>> {
        match ty.kind() {
            TyKind::Adt(def, args) if self.queries.contains(&def.did()) => args.types().next(),
            _ => None,
        }
    }

    pub fn is_heap(&self, ty: Ty<'_>) -> bool {
        match ty.kind() {
            TyKind::Adt(adt, args) => {
                self.heap_types.contains(&adt.did())
                    && !args.types().next().is_some_and(|item| item.is_unit())
            }
            _ => false,
        }
    }

    pub fn is_blocking(&self, tcx: TyCtxt<'_>, callee: DefId) -> bool {
        if self.blocking_fns.contains(&callee) {
            return true;
        }
        if self.blocking_module.is_some() && tcx.opt_parent(callee) == self.blocking_module {
            return true;
        }
        self.method_host(tcx, callee)
            .is_some_and(|did| self.blocking_types.contains(&did))
    }

    pub fn is_observer_call(&self, tcx: TyCtxt<'_>, callee: DefId) -> bool {
        matches!(tcx.item_name(callee).as_str(), "add_observer" | "observe")
            && self
                .method_host(tcx, callee)
                .is_some_and(|host| self.observer_hosts.contains(&host))
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

pub fn type_arg_of<'tcx>(ty: Ty<'tcx>, adt: DefId) -> Option<Ty<'tcx>> {
    match ty.kind() {
        TyKind::Adt(def, args) if def.did() == adt => args.types().next(),
        _ => None,
    }
}

pub fn implements<'tcx>(
    tcx: TyCtxt<'tcx>,
    owner: LocalDefId,
    trait_def: DefId,
    args: impl IntoIterator<Item = Ty<'tcx>>,
) -> bool {
    let env = TypingEnv::post_analysis(tcx, owner);
    let (infcx, param_env) = tcx.infer_ctxt().build_with_typing_env(env);
    infcx
        .type_implements_trait(trait_def, args, param_env)
        .must_apply_modulo_regions()
}

pub fn is_source_fn(tcx: TyCtxt<'_>, def: DefId) -> bool {
    matches!(tcx.def_kind(def), DefKind::Fn | DefKind::AssocFn)
        && !tcx.def_span(def).from_expansion()
}

pub fn in_trait_impl(tcx: TyCtxt<'_>, def: DefId) -> bool {
    matches!(
        tcx.def_kind(tcx.parent(def)),
        DefKind::Impl { of_trait: true }
    )
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

fn resolve_all(tcx: TyCtxt<'_>, paths: &[&str]) -> Vec<DefId> {
    paths
        .iter()
        .filter_map(|path| resolve_path(tcx, path))
        .collect()
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

pub fn in_cfg_test(tcx: TyCtxt<'_>, hir_id: HirId) -> bool {
    std::iter::once(hir_id)
        .chain(tcx.hir_parent_id_iter(hir_id))
        .any(|id| {
            tcx.hir_attrs(id).iter().any(|attr| {
                matches!(attr, Attribute::Parsed(AttributeKind::CfgTrace(cfgs))
                    if cfgs.iter().any(|(cfg, _)| requires_test(cfg)))
            })
        })
}

fn requires_test(cfg: &CfgEntry) -> bool {
    match cfg {
        CfgEntry::NameValue {
            name, value: None, ..
        } => *name == rustc_span::sym::test,
        CfgEntry::All(entries, _) => entries.iter().any(requires_test),
        CfgEntry::Any(entries, _) => entries.iter().all(requires_test),
        _ => false,
    }
}
