#![feature(rustc_private)]

extern crate rustc_ast;
extern crate rustc_data_structures;
extern crate rustc_driver;
extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_infer;
extern crate rustc_interface;
extern crate rustc_lint;
extern crate rustc_lint_defs;
extern crate rustc_middle;
extern crate rustc_session;
extern crate rustc_span;
extern crate rustc_trait_selection;

mod anchors;
mod arguments_rule;
mod body_scan;
mod callbacks;
mod collect;
mod config_node;
mod ecs_lint;
mod event_conditions;
mod facts;
mod lints;
mod resolve;
mod set_graph;
mod signature_rules;
mod system_rules;

use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::process::ExitCode;

use callbacks::LintCallbacks;
use rustc_session::EarlyDiagCtxt;
use rustc_session::config::ErrorOutputType;

const TOOL_ARGS: [&str; 2] = [
    "-Zcrate-attr=feature(register_tool)",
    "-Zcrate-attr=register_tool(mcrs)",
];

fn main() -> ExitCode {
    #[expect(
        clippy::disallowed_methods,
        reason = "rustup names the active toolchain in the environment"
    )]
    let active = std::env::var("RUSTUP_TOOLCHAIN");
    if let (Some(built), Ok(active)) = (option_env!("RUSTUP_TOOLCHAIN"), active)
        && built != active
    {
        eprintln!(
            "warning: mcrs_lint is built for {built}, the toolchain is {active}; rebuild it with `cargo build --release --manifest-path lints/Cargo.toml`"
        );
        return run_rustc();
    }
    let early = EarlyDiagCtxt::new(ErrorOutputType::default());
    rustc_driver::init_rustc_env_logger(&early);
    rustc_driver::install_ice_hook("https://github.com/andreypfau/mcrs/issues", |_| ());
    rustc_driver::catch_with_exit_code(move || {
        let mut args = rustc_driver::args::raw_args(&early);
        if is_wrapper_call(&args) {
            args.remove(0);
        }
        args.extend(TOOL_ARGS.map(str::to_owned));
        rustc_driver::run_compiler(&args, &mut LintCallbacks);
    })
}

fn is_wrapper_call(args: &[String]) -> bool {
    args.get(1)
        .and_then(|first| Path::new(first).file_stem())
        .is_some_and(|stem| stem == "rustc")
}

fn run_rustc() -> ExitCode {
    let all: Vec<String> = std::env::args().collect();
    let mut args = all.iter().skip(1);
    let rustc = if is_wrapper_call(&all) {
        match args.next() {
            Some(rustc) => PathBuf::from(rustc),
            None => return ExitCode::FAILURE,
        }
    } else {
        PathBuf::from("rustc")
    };
    Command::new(rustc)
        .args(args)
        .status()
        .ok()
        .and_then(|status| status.code())
        .and_then(|code| u8::try_from(code).ok())
        .map_or(ExitCode::FAILURE, ExitCode::from)
}
