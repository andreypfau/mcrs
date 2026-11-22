use std::sync::OnceLock;

use rustc_driver::Callbacks;
use rustc_interface::interface::Config;
use rustc_lint_defs::RegisteredTools;
use rustc_middle::ty::TyCtxt;
use rustc_span::Ident;
use rustc_span::Symbol;

use crate::ecs_lint::EcsLint;
use crate::rules;

type RegisteredToolsQuery = for<'tcx> fn(TyCtxt<'tcx>, ()) -> RegisteredTools;

static REGISTERED_TOOLS: OnceLock<RegisteredToolsQuery> = OnceLock::new();

const TRACKED_ENV: [&str; 2] = ["MCRS_LINT_TICK_SCHEDULES", "MCRS_LINT_DEBUG"];

pub struct LintCallbacks;

impl Callbacks for LintCallbacks {
    fn config(&mut self, config: &mut Config) {
        config.crate_cfg.push("mcrs_lint".to_owned());
        config.crate_check_cfg.push("cfg(mcrs_lint)".to_owned());
        config.register_lints = Some(Box::new(|_, store| {
            store.register_lints(&rules::LINTS);
            store.register_late_lint_pass(Box::new(|_| Box::new(EcsLint)));
        }));
        config.track_state = Some(Box::new(|sess| {
            if let Some(driver) = std::env::current_exe()
                .ok()
                .and_then(|path| path.to_str().map(Symbol::intern))
            {
                sess.file_depinfo.borrow_mut().insert(driver);
            }
            let mut env = sess.env_depinfo.borrow_mut();
            #[expect(
                clippy::disallowed_methods,
                reason = "cargo passes the lint configuration from `[env]` to the driver"
            )]
            let values = TRACKED_ENV.map(|var| (var, std::env::var(var).ok()));
            for (var, value) in values {
                env.insert((
                    Symbol::intern(var),
                    value.map(|value| Symbol::intern(&value)),
                ));
            }
        }));
        config.override_queries = Some(|_, providers| {
            let _ = REGISTERED_TOOLS.set(providers.queries.registered_lint_tools);
            providers.queries.registered_lint_tools = registered_tools;
        });
    }
}

fn registered_tools(tcx: TyCtxt<'_>, (): ()) -> RegisteredTools {
    let mut tools = REGISTERED_TOOLS
        .get()
        .map(|original| original(tcx, ()))
        .unwrap_or_default();
    tools.insert(Ident::from_str("mcrs"));
    tools
}
