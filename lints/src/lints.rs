use rustc_errors::Diag;
use rustc_errors::DiagDecorator;
use rustc_errors::MultiSpan;
use rustc_hir::HirId;
use rustc_lint::Level;
use rustc_lint::Lint;
use rustc_middle::ty::TyCtxt;

const fn lint(name: &'static str, desc: &'static str) -> Lint {
    Lint {
        name,
        default_level: Level::Deny,
        desc,
        is_externally_loaded: true,
        eval_always: true,
        ..Lint::default_fields_for_macro()
    }
}

pub static EXCLUSIVE_SYSTEM: &Lint = &lint(
    "mcrs::exclusive_system",
    "a system takes `&mut World` and runs alone",
);
pub static EXCLUSIVE_OBSERVER: &Lint = &lint(
    "mcrs::exclusive_observer",
    "an observer takes `&mut World`, which panics when the observer is added",
);
pub static UNGATED_SYSTEM: &Lint = &lint(
    "mcrs::ungated_system",
    "a system in a tick schedule has no run condition",
);
pub static BLOCKING_IN_SYSTEM: &Lint = &lint(
    "mcrs::blocking_in_system",
    "a tick system reads the wall clock, sleeps, blocks or does file I/O",
);
pub static ALLOC_IN_TICK: &Lint = &lint(
    "mcrs::alloc_in_tick",
    "a tick system allocates a heap collection on each run",
);
pub static ALLOC_SITE_COUNT: &Lint = &lint(
    "mcrs::alloc_site_count",
    "the expectation of `alloc_in_tick` states another number of sites than the system has",
);
pub static PER_FRAME_INSERT_RESOURCE: &Lint = &lint(
    "mcrs::per_frame_insert_resource",
    "a tick system inserts a resource on each run",
);
pub static MARKER_TOGGLE: &Lint = &lint(
    "mcrs::marker_toggle",
    "a tick system inserts and removes the same marker component",
);
pub static TIME_CLOCK_MISMATCH: &Lint = &lint(
    "mcrs::time_clock_mismatch",
    "a system in a fixed schedule reads the real or the virtual clock",
);
pub static MISSED_SET_IF_NEQ: &Lint = &lint(
    "mcrs::missed_set_if_neq",
    "a write through `DerefMut` of a change-tracked value without a comparison",
);
pub static ZST_QUERY: &Lint = &lint(
    "mcrs::zst_query",
    "a query fetches a marker component as data",
);
pub static BORROWED_REBORROWABLE: &Lint = &lint(
    "mcrs::borrowed_reborrowable",
    "a parameter takes `&mut` of a type that has `reborrow`",
);
pub static INSERT_MESSAGE_RESOURCE: &Lint = &lint(
    "mcrs::insert_message_resource",
    "`Messages<T>` is inserted as a plain resource",
);

pub static TOO_MANY_ARGUMENTS: &Lint = &lint(
    "mcrs::too_many_arguments",
    "a function has more than seven parameters that are not system parameters",
);
pub static UNIT_IN_BUNDLE: &Lint = &lint("mcrs::unit_in_bundle", "a bundle contains `()`");
pub static ALWAYS_CHANGED_FILTER: &Lint = &lint(
    "mcrs::always_changed_filter",
    "a `Changed<T>` filter on a component that a tick system writes on each run",
);

pub static INVALID_EVENT_CONDITION: &Lint = &lint(
    "mcrs::invalid_event_condition",
    "`mcrs::event_condition` marks an item that is not a run condition",
);

pub static LINTS: [&Lint; 17] = [
    EXCLUSIVE_SYSTEM,
    EXCLUSIVE_OBSERVER,
    UNGATED_SYSTEM,
    BLOCKING_IN_SYSTEM,
    ALLOC_IN_TICK,
    ALLOC_SITE_COUNT,
    PER_FRAME_INSERT_RESOURCE,
    MARKER_TOGGLE,
    TIME_CLOCK_MISMATCH,
    MISSED_SET_IF_NEQ,
    ZST_QUERY,
    BORROWED_REBORROWABLE,
    INSERT_MESSAGE_RESOURCE,
    TOO_MANY_ARGUMENTS,
    UNIT_IN_BUNDLE,
    ALWAYS_CHANGED_FILTER,
    INVALID_EVENT_CONDITION,
];

pub fn emit(
    tcx: TyCtxt<'_>,
    lint: &'static Lint,
    hir_id: HirId,
    span: impl Into<MultiSpan>,
    decorate: impl FnOnce(&mut Diag<'_, ()>),
) {
    tcx.emit_node_span_lint(lint, hir_id, span, DiagDecorator(decorate));
}
